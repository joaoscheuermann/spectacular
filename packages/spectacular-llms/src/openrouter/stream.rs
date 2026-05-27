use super::client::OpenRouterHttpClient;
use super::debug;
use super::dto::OpenRouterChatRequest;
use super::parser::{parse_openrouter_chat_chunk_with_accumulator, OpenRouterToolCallAccumulator};
use super::sse::OpenRouterSseParser;
use crate::{
    Cancellation, FinishReason, LlmDebugLogger, ProviderError, ProviderErrorDiagnostics,
    ProviderErrorStage, ProviderFinished, ProviderRequest, ProviderStream, ProviderStreamEvent,
};
use serde_json::json;
use tokio::sync::mpsc;

pub(crate) async fn openrouter_stream_completion(
    api_key: String,
    client: OpenRouterHttpClient,
    debug_logger: LlmDebugLogger,
    request: ProviderRequest,
    cancellation: Cancellation,
) -> Result<ProviderStream, ProviderError> {
    if cancellation.is_cancelled() {
        debug::log_event(&debug_logger, "stream_cancelled_before_start", json!({}));
        return Err(ProviderError::CancellationError);
    }
    if api_key.trim().is_empty() {
        debug::log_error(
            &debug_logger,
            "stream_invalid_api_key",
            &ProviderError::InvalidApiKey,
        );
        return Err(ProviderError::InvalidApiKey);
    }

    let (sender, receiver) = mpsc::channel(128);
    tokio::spawn(async move {
        let result = stream_openrouter_response(
            &client,
            &api_key,
            &debug_logger,
            request,
            cancellation,
            sender.clone(),
        )
        .await;
        if let Err(error) = result {
            debug::log_error(&debug_logger, "stream_error", &error);
            let _ = sender.send(Err(error)).await;
        }
    });

    Ok(ProviderStream::new(receiver))
}

async fn stream_openrouter_response(
    client: &OpenRouterHttpClient,
    api_key: &str,
    debug_logger: &LlmDebugLogger,
    request: ProviderRequest,
    cancellation: Cancellation,
    sender: mpsc::Sender<Result<ProviderStreamEvent, ProviderError>>,
) -> Result<(), ProviderError> {
    let body = build_chat_request(request, debug_logger)?;
    let response = open_chat_response(client, api_key, debug_logger, &body).await?;
    let mut response = successful_response_or_error(debug_logger, response).await?;
    let context = OpenRouterStreamContext {
        debug_logger,
        sender: &sender,
    };

    stream_successful_response(&mut response, cancellation, context).await
}

fn build_chat_request(
    request: ProviderRequest,
    debug_logger: &LlmDebugLogger,
) -> Result<OpenRouterChatRequest, ProviderError> {
    let body = OpenRouterChatRequest::from_provider_request(request).inspect_err(|error| {
        debug::log_error(debug_logger, "chat_request_build_error", error);
    })?;
    if let Ok(raw_json) = serde_json::to_value(&body) {
        debug::log_raw_json(debug_logger, "chat_request", raw_json);
    }

    Ok(body)
}

async fn open_chat_response(
    client: &OpenRouterHttpClient,
    api_key: &str,
    debug_logger: &LlmDebugLogger,
    body: &OpenRouterChatRequest,
) -> Result<reqwest::Response, ProviderError> {
    client
        .stream_response(api_key, body)
        .await
        .inspect_err(|error| {
            debug::log_error(debug_logger, "chat_request_network_error", error);
        })
}

async fn successful_response_or_error(
    debug_logger: &LlmDebugLogger,
    response: reqwest::Response,
) -> Result<reqwest::Response, ProviderError> {
    let status = response.status().as_u16();
    debug::log_event(
        debug_logger,
        "chat_response_status",
        json!({ "status": status }),
    );
    if status == 401 || status == 403 {
        let diagnostics = non_success_response_diagnostics(debug_logger, response, status).await;
        return Err(ProviderError::AuthenticationFailed {
            provider_name: "OpenRouter".to_owned(),
            reason: format!("credentials rejected with status {status}"),
            diagnostics: Some(diagnostics.boxed()),
        });
    }
    if !(200..300).contains(&status) {
        let diagnostics = non_success_response_diagnostics(debug_logger, response, status).await;
        return Err(ProviderError::ProviderUnavailable {
            provider_name: "OpenRouter".to_owned(),
            diagnostics: Some(diagnostics.boxed()),
        });
    }

    Ok(response)
}

#[derive(Clone, Copy)]
struct OpenRouterStreamContext<'a> {
    debug_logger: &'a LlmDebugLogger,
    sender: &'a mpsc::Sender<Result<ProviderStreamEvent, ProviderError>>,
}

async fn stream_successful_response(
    response: &mut reqwest::Response,
    cancellation: Cancellation,
    context: OpenRouterStreamContext<'_>,
) -> Result<(), ProviderError> {
    let mut sse_parser = OpenRouterSseParser::default();
    let mut saw_finished = false;
    let mut stream_state = OpenRouterStreamState::default();
    while let Some(chunk) = next_response_chunk(response, context.debug_logger).await? {
        if cancellation.is_cancelled() {
            debug::log_event(context.debug_logger, "stream_cancelled", json!({}));
            return Err(ProviderError::CancellationError);
        }

        if process_response_chunk(
            &mut sse_parser,
            &chunk,
            &mut stream_state,
            &mut saw_finished,
            context,
        )
        .await?
        {
            return Ok(());
        }
    }

    finish_stream_if_needed(&mut stream_state, saw_finished, context).await
}

async fn process_response_chunk(
    sse_parser: &mut OpenRouterSseParser,
    chunk: &[u8],
    state: &mut OpenRouterStreamState,
    saw_finished: &mut bool,
    context: OpenRouterStreamContext<'_>,
) -> Result<bool, ProviderError> {
    for payload in parse_sse_payloads(sse_parser, chunk, context.debug_logger)? {
        debug::log_raw_text(context.debug_logger, "sse_payload", &payload);
        if handle_payload(&payload, state, saw_finished, context).await? {
            return Ok(true);
        }
    }

    Ok(false)
}

async fn handle_payload(
    payload: &str,
    state: &mut OpenRouterStreamState,
    saw_finished: &mut bool,
    context: OpenRouterStreamContext<'_>,
) -> Result<bool, ProviderError> {
    if payload.trim() == "[DONE]" {
        return finish_done_payload(state, *saw_finished, context).await;
    }

    let finished_in_payload =
        send_openrouter_payload_events(payload, state, context.debug_logger, context.sender)
            .await?;
    *saw_finished |= finished_in_payload;
    Ok(finished_in_payload)
}

async fn finish_done_payload(
    state: &mut OpenRouterStreamState,
    saw_finished: bool,
    context: OpenRouterStreamContext<'_>,
) -> Result<bool, ProviderError> {
    debug::log_event(context.debug_logger, "sse_done", json!({}));
    if state.has_pending_tool_call() {
        return Err(pending_tool_call_error());
    }
    if !saw_finished {
        let finished = state
            .take_pending_finish()
            .unwrap_or_else(ProviderFinished::stopped);
        debug::log_finish(context.debug_logger, "stream_finished", &finished);
        send_openrouter_event(ProviderStreamEvent::Finished(finished), context.sender).await?;
    }
    Ok(true)
}

async fn finish_stream_if_needed(
    state: &mut OpenRouterStreamState,
    saw_finished: bool,
    context: OpenRouterStreamContext<'_>,
) -> Result<(), ProviderError> {
    if !saw_finished && state.has_pending_tool_call() {
        return Err(pending_tool_call_error());
    }

    if saw_finished {
        return Ok(());
    }

    let finished = state
        .take_pending_finish()
        .unwrap_or_else(ProviderFinished::stopped);
    debug::log_finish(context.debug_logger, "stream_finished", &finished);
    send_openrouter_event(ProviderStreamEvent::Finished(finished), context.sender).await
}

fn pending_tool_call_error() -> ProviderError {
    ProviderError::MalformedResponse {
        provider_name: "OpenRouter".to_owned(),
        reason: "stream ended before tool-call finish".to_owned(),
        diagnostics: Some(
            ProviderErrorDiagnostics::new(ProviderErrorStage::ProviderStream).boxed(),
        ),
    }
}

async fn next_response_chunk(
    response: &mut reqwest::Response,
    debug_logger: &LlmDebugLogger,
) -> Result<Option<Vec<u8>>, ProviderError> {
    response
        .chunk()
        .await
        .map_err(|error| {
            let error = ProviderError::NetworkError {
                provider_name: "OpenRouter".to_owned(),
                reason: error.to_string(),
                diagnostics: Some(
                    ProviderErrorDiagnostics::new(ProviderErrorStage::ProviderStream)
                        .with_debug_event("stream_chunk_network_error")
                        .boxed(),
                ),
            };
            debug::log_error(debug_logger, "stream_chunk_network_error", &error);
            error
        })
        .map(|chunk| chunk.map(|chunk| chunk.to_vec()))
}

fn parse_sse_payloads(
    sse_parser: &mut OpenRouterSseParser,
    chunk: &[u8],
    debug_logger: &LlmDebugLogger,
) -> Result<Vec<String>, ProviderError> {
    sse_parser.push(chunk).inspect_err(|error| {
        debug::log_error(debug_logger, "sse_parse_error", error);
    })
}

async fn non_success_response_diagnostics(
    debug_logger: &LlmDebugLogger,
    response: reqwest::Response,
    status: u16,
) -> ProviderErrorDiagnostics {
    let diagnostics =
        ProviderErrorDiagnostics::new(ProviderErrorStage::HttpStatus).with_http_status(status);
    match response.text().await {
        Ok(body) => {
            debug::log_raw_text(debug_logger, "chat_response_error_body", &body);
            http_status_body_diagnostics(status, &body, "chat_response_error_body")
        }
        Err(error) => {
            debug::log_event(
                debug_logger,
                "chat_response_error_body_read_failed",
                json!({ "message": error.to_string() }),
            );
            diagnostics.with_debug_event("chat_response_error_body_read_failed")
        }
    }
}

fn http_status_body_diagnostics(
    status: u16,
    body: &str,
    debug_event: &str,
) -> ProviderErrorDiagnostics {
    ProviderErrorDiagnostics::new(ProviderErrorStage::HttpStatus)
        .with_http_status(status)
        .with_provider_code_from_body(body)
        .with_excerpt(body)
        .with_debug_event(debug_event)
}

#[derive(Default)]
struct OpenRouterStreamState {
    tool_call_accumulator: OpenRouterToolCallAccumulator,
    pending_finish: Option<ProviderFinished>,
}

impl OpenRouterStreamState {
    fn has_pending_tool_call(&self) -> bool {
        self.tool_call_accumulator.has_pending()
    }

    fn take_pending_finish(&mut self) -> Option<ProviderFinished> {
        self.pending_finish.take()
    }
}

async fn send_openrouter_payload_events(
    payload: &str,
    state: &mut OpenRouterStreamState,
    debug_logger: &LlmDebugLogger,
    sender: &mpsc::Sender<Result<ProviderStreamEvent, ProviderError>>,
) -> Result<bool, ProviderError> {
    let events =
        parse_openrouter_chat_chunk_with_accumulator(payload, &mut state.tool_call_accumulator)
            .inspect_err(|error| {
                debug::log_error(debug_logger, "payload_parse_error", error);
            })?;

    for event in events {
        let ProviderStreamEvent::Finished(finished) = event else {
            if state.pending_finish.is_some() {
                return Err(ProviderError::MalformedResponse {
                    provider_name: "OpenRouter".to_owned(),
                    reason: "OpenRouter emitted content after a terminal finish".to_owned(),
                    diagnostics: Some(
                        ProviderErrorDiagnostics::new(ProviderErrorStage::PayloadParse)
                            .with_excerpt(payload)
                            .with_debug_event("sse_payload")
                            .with_debug_event("payload_parse_error")
                            .boxed(),
                    ),
                });
            }
            send_openrouter_event(event, sender).await?;
            continue;
        };

        if finished.finish_reason == FinishReason::ToolCalls {
            debug::log_finish(debug_logger, "stream_finished", &finished);
            send_openrouter_event(ProviderStreamEvent::Finished(finished), sender).await?;
            return Ok(true);
        }

        if finished.usage.is_some() {
            let finished = if let Some(pending) = state.pending_finish.take() {
                merge_openrouter_final_usage(pending, finished)
            } else {
                finished
            };
            debug::log_finish(debug_logger, "stream_finished", &finished);
            send_openrouter_event(ProviderStreamEvent::Finished(finished), sender).await?;
            return Ok(true);
        }

        state.pending_finish = Some(finished);
    }

    Ok(false)
}

fn merge_openrouter_final_usage(
    mut pending: ProviderFinished,
    usage_finish: ProviderFinished,
) -> ProviderFinished {
    if pending.usage.is_none() {
        pending.usage = usage_finish.usage;
    }
    if pending.reasoning.is_none() {
        pending.reasoning = usage_finish.reasoning;
    }
    pending
}

async fn send_openrouter_event(
    event: ProviderStreamEvent,
    sender: &mpsc::Sender<Result<ProviderStreamEvent, ProviderError>>,
) -> Result<(), ProviderError> {
    sender
        .send(Ok(event))
        .await
        .map_err(|_| ProviderError::CancellationError)
}

#[cfg(test)]
mod tests {
    use super::super::dto::OpenRouterChatRequest;
    use super::super::models::{fetch_openrouter_models, validate_openrouter_api_key};
    use super::super::parser::parse_openrouter_chat_chunk;
    use super::super::sse::OpenRouterSseParser;
    use super::super::OpenRouterProvider;
    use super::*;
    use crate::{
        LlmDebugLogger, LlmProvider, MessageDelta, Model, ProviderCapabilities,
        ProviderContextLimits, ProviderMessage, ProviderMessageRole, ProviderToolCall,
        ReasoningDelta, ToolManifest, UsageMetadata,
    };
    use serde_json::json;

    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/openrouter_models_and_requests.rs"
    ));
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/openrouter_stream.rs"
    ));
}
