use super::client::OpenRouterHttpClient;
use super::debug;
use super::dto::OpenRouterChatRequest;
use super::parser::{parse_openrouter_chat_chunk_with_accumulator, OpenRouterToolCallAccumulator};
use super::sse::OpenRouterSseParser;
use crate::{
    Cancellation, FinishReason, LlmDebugLogger, ProviderError, ProviderFinished, ProviderRequest,
    ProviderStream, ProviderStreamEvent,
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
    let body = OpenRouterChatRequest::from_provider_request(request).map_err(|error| {
        debug::log_error(debug_logger, "chat_request_build_error", &error);
        error
    })?;
    if let Ok(raw_json) = serde_json::to_value(&body) {
        debug::log_raw_json(debug_logger, "chat_request", raw_json);
    }

    let mut response = client
        .stream_response(api_key, &body)
        .await
        .map_err(|error| {
            debug::log_error(debug_logger, "chat_request_network_error", &error);
            error
        })?;

    let status = response.status().as_u16();
    debug::log_event(
        debug_logger,
        "chat_response_status",
        json!({ "status": status }),
    );
    if status == 401 || status == 403 {
        debug::log_error(
            debug_logger,
            "chat_response_invalid_api_key",
            &ProviderError::InvalidApiKey,
        );
        return Err(ProviderError::InvalidApiKey);
    }
    if !(200..300).contains(&status) {
        log_non_success_response_body(debug_logger, response).await;
        return Err(ProviderError::ProviderUnavailable {
            provider_name: "OpenRouter".to_owned(),
        });
    }

    let mut sse_parser = OpenRouterSseParser::default();
    let mut saw_finished = false;
    let mut stream_state = OpenRouterStreamState::default();
    while let Some(chunk) = next_response_chunk(&mut response, debug_logger).await? {
        if cancellation.is_cancelled() {
            debug::log_event(debug_logger, "stream_cancelled", json!({}));
            return Err(ProviderError::CancellationError);
        }

        for payload in parse_sse_payloads(&mut sse_parser, &chunk, debug_logger)? {
            debug::log_raw_text(debug_logger, "sse_payload", &payload);
            if payload.trim() == "[DONE]" {
                debug::log_event(debug_logger, "sse_done", json!({}));
                if stream_state.has_pending_tool_call() {
                    return Err(ProviderError::MalformedResponse {
                        provider_name: "OpenRouter".to_owned(),
                        reason: "stream ended before tool-call finish".to_owned(),
                    });
                }
                if !saw_finished {
                    let finished = stream_state
                        .take_pending_finish()
                        .unwrap_or_else(ProviderFinished::stopped);
                    debug::log_finish(debug_logger, "stream_finished", &finished);
                    send_openrouter_event(ProviderStreamEvent::Finished(finished), &sender).await?;
                }
                return Ok(());
            }

            let finished_in_payload =
                send_openrouter_payload_events(&payload, &mut stream_state, debug_logger, &sender)
                    .await?;
            saw_finished |= finished_in_payload;
            if finished_in_payload {
                return Ok(());
            }
        }
    }

    if !saw_finished && stream_state.has_pending_tool_call() {
        return Err(ProviderError::MalformedResponse {
            provider_name: "OpenRouter".to_owned(),
            reason: "stream ended before tool-call finish".to_owned(),
        });
    }

    if !saw_finished {
        let finished = stream_state
            .take_pending_finish()
            .unwrap_or_else(ProviderFinished::stopped);
        debug::log_finish(debug_logger, "stream_finished", &finished);
        send_openrouter_event(ProviderStreamEvent::Finished(finished), &sender).await?;
    }

    Ok(())
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
    sse_parser.push(chunk).map_err(|error| {
        debug::log_error(debug_logger, "sse_parse_error", &error);
        error
    })
}

async fn log_non_success_response_body(debug_logger: &LlmDebugLogger, response: reqwest::Response) {
    match response.text().await {
        Ok(body) => debug::log_raw_text(debug_logger, "chat_response_error_body", &body),
        Err(error) => debug::log_event(
            debug_logger,
            "chat_response_error_body_read_failed",
            json!({ "message": error.to_string() }),
        ),
    }
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
            .map_err(|error| {
                debug::log_error(debug_logger, "payload_parse_error", &error);
                error
            })?;

    for event in events {
        let ProviderStreamEvent::Finished(finished) = event else {
            if state.pending_finish.is_some() {
                return Err(ProviderError::MalformedResponse {
                    provider_name: "OpenRouter".to_owned(),
                    reason: format!(
                        "OpenRouter emitted content after a terminal finish; OpenRouter response chunk JSON: {payload}"
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
