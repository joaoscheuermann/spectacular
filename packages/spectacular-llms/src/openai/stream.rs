use super::auth::{refresh_openai_auth, OpenAiAuthRecord, OpenAiAuthStore};
use super::client::OpenAiHttpClient;
use super::debug;
use super::dto::OpenAiResponsesRequest;
use super::parser::parse_openai_response_event;
use super::sse::OpenAiSseParser;
use super::OpenAiProviderAuth;
use crate::{
    Cancellation, LlmDebugLogger, ProviderError, ProviderErrorDiagnostics, ProviderErrorStage,
    ProviderFinished, ProviderRequest, ProviderStream, ProviderStreamEvent,
};
use serde_json::json;
use tokio::sync::mpsc;

/// Starts an OpenAI streaming completion and returns a provider stream handle.
pub(crate) async fn openai_stream_completion(
    auth: OpenAiProviderAuth,
    client: OpenAiHttpClient,
    debug_logger: LlmDebugLogger,
    request: ProviderRequest,
    cancellation: Cancellation,
) -> Result<ProviderStream, ProviderError> {
    if cancellation.is_cancelled() {
        debug::log_event(&debug_logger, "stream_cancelled_before_start", json!({}));
        return Err(ProviderError::CancellationError);
    }

    let (sender, receiver) = mpsc::channel(128);
    tokio::spawn(async move {
        let result = stream_openai_response(
            auth,
            client,
            debug_logger.clone(),
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

/// Streams a single OpenAI Responses request, including auth refresh and retry-once for 401.
async fn stream_openai_response(
    auth: OpenAiProviderAuth,
    client: OpenAiHttpClient,
    debug_logger: LlmDebugLogger,
    request: ProviderRequest,
    cancellation: Cancellation,
    sender: mpsc::Sender<Result<ProviderStreamEvent, ProviderError>>,
) -> Result<(), ProviderError> {
    let body = build_responses_request(request, &debug_logger)?;
    let response = open_authenticated_response(&auth, &client, &debug_logger, &body).await?;
    let mut response = successful_response_or_error(&debug_logger, response).await?;
    let context = OpenAiStreamContext {
        debug_logger: &debug_logger,
        sender: &sender,
    };

    stream_successful_response(&mut response, cancellation, context).await
}

fn build_responses_request(
    request: ProviderRequest,
    debug_logger: &LlmDebugLogger,
) -> Result<OpenAiResponsesRequest, ProviderError> {
    let body = OpenAiResponsesRequest::from_provider_request(request)?;
    if let Ok(raw_json) = serde_json::to_value(&body) {
        debug::log_raw_json(debug_logger, "responses_request", raw_json);
    }

    Ok(body)
}

async fn successful_response_or_error(
    debug_logger: &LlmDebugLogger,
    response: reqwest::Response,
) -> Result<reqwest::Response, ProviderError> {
    let status = response.status().as_u16();
    debug::log_event(
        debug_logger,
        "responses_status",
        json!({ "status": status }),
    );

    if status == 401 || status == 403 {
        let diagnostics = non_success_response_diagnostics(debug_logger, response, status).await;
        return Err(ProviderError::AuthenticationFailed {
            provider_name: "OpenAI".to_owned(),
            reason: format!("credentials rejected with status {status}"),
            diagnostics: Some(diagnostics.boxed()),
        });
    }
    if !(200..300).contains(&status) {
        let diagnostics = non_success_response_diagnostics(debug_logger, response, status).await;
        return Err(ProviderError::ProviderUnavailable {
            provider_name: "OpenAI".to_owned(),
            diagnostics: Some(diagnostics.boxed()),
        });
    }

    Ok(response)
}

#[derive(Clone, Copy)]
struct OpenAiStreamContext<'a> {
    debug_logger: &'a LlmDebugLogger,
    sender: &'a mpsc::Sender<Result<ProviderStreamEvent, ProviderError>>,
}

async fn stream_successful_response(
    response: &mut reqwest::Response,
    cancellation: Cancellation,
    context: OpenAiStreamContext<'_>,
) -> Result<(), ProviderError> {
    let mut parser = OpenAiSseParser::default();
    while let Some(chunk) = next_response_chunk(response, context.debug_logger).await? {
        if cancellation.is_cancelled() {
            debug::log_event(context.debug_logger, "stream_cancelled", json!({}));
            return Err(ProviderError::CancellationError);
        }

        if process_response_chunk(&mut parser, &chunk, context).await? {
            return Ok(());
        }
    }

    send_stopped_event(context).await
}

async fn process_response_chunk(
    parser: &mut OpenAiSseParser,
    chunk: &[u8],
    context: OpenAiStreamContext<'_>,
) -> Result<bool, ProviderError> {
    for payload in parser.push(chunk)? {
        debug::log_raw_text(context.debug_logger, "sse_payload", &payload);
        if handle_payload(&payload, context).await? {
            return Ok(true);
        }
    }

    Ok(false)
}

async fn handle_payload(
    payload: &str,
    context: OpenAiStreamContext<'_>,
) -> Result<bool, ProviderError> {
    if payload.trim() == "[DONE]" {
        send_stopped_event(context).await?;
        return Ok(true);
    }

    send_payload_events(payload, context.debug_logger, context.sender).await
}

async fn send_stopped_event(context: OpenAiStreamContext<'_>) -> Result<(), ProviderError> {
    send_openai_event(
        ProviderStreamEvent::Finished(ProviderFinished::stopped()),
        context.sender,
    )
    .await
}

/// Opens the correct OpenAI backend for the configured auth mode.
async fn open_authenticated_response(
    auth: &OpenAiProviderAuth,
    client: &OpenAiHttpClient,
    debug_logger: &LlmDebugLogger,
    body: &OpenAiResponsesRequest,
) -> Result<reqwest::Response, ProviderError> {
    match auth {
        OpenAiProviderAuth::ApiKey(api_key) => {
            if api_key.trim().is_empty() {
                return Err(ProviderError::AuthenticationRequired {
                    provider_name: "OpenAI".to_owned(),
                });
            }

            client.stream_api_response(api_key, body).await
        }
        OpenAiProviderAuth::Oauth(auth_store) => {
            open_chatgpt_response(auth_store.as_ref(), client, debug_logger, body).await
        }
    }
}

/// Opens a ChatGPT-authenticated response, refreshing once before returning a 401.
async fn open_chatgpt_response(
    auth_store: &dyn OpenAiAuthStore,
    client: &OpenAiHttpClient,
    debug_logger: &LlmDebugLogger,
    body: &OpenAiResponsesRequest,
) -> Result<reqwest::Response, ProviderError> {
    let auth = load_fresh_auth(auth_store, client, debug_logger).await?;
    let mut response = client.stream_chatgpt_response(&auth, body).await?;
    if response.status().as_u16() != 401 {
        return Ok(response);
    }

    let auth = refresh_after_unauthorized(auth_store, client, auth).await?;
    response = client.stream_chatgpt_response(&auth, body).await?;
    debug::log_event(
        debug_logger,
        "responses_status_after_refresh",
        json!({ "status": response.status().as_u16() }),
    );
    Ok(response)
}

/// Loads auth and refreshes it before use when the cached credentials are stale.
async fn load_fresh_auth(
    auth_store: &dyn OpenAiAuthStore,
    client: &OpenAiHttpClient,
    debug_logger: &LlmDebugLogger,
) -> Result<OpenAiAuthRecord, ProviderError> {
    let auth = auth_store.load_openai_auth()?;
    if !auth.should_refresh(super::auth::unix_timestamp()) {
        return Ok(auth);
    }

    debug::log_event(debug_logger, "auth_refresh_before_request", json!({}));
    let refreshed = refresh_openai_auth(client, auth).await?;
    auth_store.save_openai_auth(refreshed.clone())?;
    Ok(refreshed)
}

/// Refreshes credentials after a 401, reusing externally changed auth when possible.
async fn refresh_after_unauthorized(
    auth_store: &dyn OpenAiAuthStore,
    client: &OpenAiHttpClient,
    current: OpenAiAuthRecord,
) -> Result<OpenAiAuthRecord, ProviderError> {
    let reloaded = auth_store.load_openai_auth()?;
    if reloaded.access_token != current.access_token && reloaded.account_id == current.account_id {
        return Ok(reloaded);
    }

    let refreshed = refresh_openai_auth(client, current).await?;
    auth_store.save_openai_auth(refreshed.clone())?;
    Ok(refreshed)
}

/// Reads the next response chunk from a streaming request.
async fn next_response_chunk(
    response: &mut reqwest::Response,
    debug_logger: &LlmDebugLogger,
) -> Result<Option<Vec<u8>>, ProviderError> {
    response
        .chunk()
        .await
        .map_err(|error| {
            let error = ProviderError::NetworkError {
                provider_name: "OpenAI".to_owned(),
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

/// Sends parsed provider events and reports whether a terminal event was seen.
async fn send_payload_events(
    payload: &str,
    debug_logger: &LlmDebugLogger,
    sender: &mpsc::Sender<Result<ProviderStreamEvent, ProviderError>>,
) -> Result<bool, ProviderError> {
    let events = parse_openai_response_event(payload).inspect_err(|error| {
        debug::log_error(debug_logger, "payload_parse_error", error);
    })?;
    let mut finished = false;
    for event in events {
        finished |= matches!(event, ProviderStreamEvent::Finished(_));
        send_openai_event(event, sender).await?;
    }

    Ok(finished)
}

/// Sends one provider event through the stream channel.
async fn send_openai_event(
    event: ProviderStreamEvent,
    sender: &mpsc::Sender<Result<ProviderStreamEvent, ProviderError>>,
) -> Result<(), ProviderError> {
    sender
        .send(Ok(event))
        .await
        .map_err(|_| ProviderError::CancellationError)
}

/// Logs and summarizes a non-success response body for HTTP status diagnostics.
async fn non_success_response_diagnostics(
    debug_logger: &LlmDebugLogger,
    response: reqwest::Response,
    status: u16,
) -> ProviderErrorDiagnostics {
    let diagnostics =
        ProviderErrorDiagnostics::new(ProviderErrorStage::HttpStatus).with_http_status(status);
    match response.text().await {
        Ok(body) => {
            debug::log_raw_text(debug_logger, "responses_error_body", &body);
            http_status_body_diagnostics(status, &body, "responses_error_body")
        }
        Err(error) => {
            debug::log_event(
                debug_logger,
                "responses_error_body_read_failed",
                json!({ "message": error.to_string() }),
            );
            diagnostics.with_debug_event("responses_error_body_read_failed")
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

#[cfg(test)]
mod tests {
    use super::*;

    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/openai_stream_transport.rs"
    ));
}
