use super::client::OpenRouterHttpClient;
use super::dto::{
    OpenRouterChatChunk, OpenRouterChatDeltaToolCall, OpenRouterChatRequest,
    OpenRouterModelsResponse, OpenRouterStreamError,
};
use super::sse::OpenRouterSseParser;
use crate::{
    Cancellation, FinishReason, MessageDelta, Model, ProviderError, ProviderFinished,
    ProviderRequest, ProviderStream, ProviderStreamEvent, ProviderToolCall, ReasoningDelta,
};
use std::collections::BTreeMap;
use tokio::sync::mpsc;

pub(crate) async fn openrouter_stream_completion(
    api_key: String,
    client: OpenRouterHttpClient,
    request: ProviderRequest,
    cancellation: Cancellation,
) -> Result<ProviderStream, ProviderError> {
    if cancellation.is_cancelled() {
        return Err(ProviderError::CancellationError);
    }
    if api_key.trim().is_empty() {
        return Err(ProviderError::InvalidApiKey);
    }

    let (sender, receiver) = mpsc::channel(128);
    tokio::spawn(async move {
        let result =
            stream_openrouter_response(&client, &api_key, request, cancellation, sender.clone())
                .await;
        if let Err(error) = result {
            let _ = sender.send(Err(error)).await;
        }
    });

    Ok(ProviderStream::new(receiver))
}

async fn stream_openrouter_response(
    client: &OpenRouterHttpClient,
    api_key: &str,
    request: ProviderRequest,
    cancellation: Cancellation,
    sender: mpsc::Sender<Result<ProviderStreamEvent, ProviderError>>,
) -> Result<(), ProviderError> {
    let body = OpenRouterChatRequest::from_provider_request(request)?;
    let mut response = client.stream_response(api_key, &body).await?;

    let status = response.status().as_u16();
    if status == 401 || status == 403 {
        return Err(ProviderError::InvalidApiKey);
    }
    if !(200..300).contains(&status) {
        return Err(ProviderError::ProviderUnavailable {
            provider_name: "OpenRouter".to_owned(),
        });
    }

    let mut sse_parser = OpenRouterSseParser::default();
    let mut saw_finished = false;
    let mut stream_state = OpenRouterStreamState::default();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| ProviderError::NetworkError {
            provider_name: "OpenRouter".to_owned(),
            reason: error.to_string(),
        })?
    {
        if cancellation.is_cancelled() {
            return Err(ProviderError::CancellationError);
        }

        for payload in sse_parser.push(&chunk)? {
            if payload.trim() == "[DONE]" {
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
                    send_openrouter_event(ProviderStreamEvent::Finished(finished), &sender).await?;
                }
                return Ok(());
            }

            let finished_in_payload =
                send_openrouter_payload_events(&payload, &mut stream_state, &sender).await?;
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
        send_openrouter_event(ProviderStreamEvent::Finished(finished), &sender).await?;
    }

    Ok(())
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
    sender: &mpsc::Sender<Result<ProviderStreamEvent, ProviderError>>,
) -> Result<bool, ProviderError> {
    for event in
        parse_openrouter_chat_chunk_with_accumulator(payload, &mut state.tool_call_accumulator)?
    {
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
            send_openrouter_event(ProviderStreamEvent::Finished(finished), sender).await?;
            return Ok(true);
        }

        if finished.usage.is_some() {
            let finished = if let Some(pending) = state.pending_finish.take() {
                merge_openrouter_final_usage(pending, finished)
            } else {
                finished
            };
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
fn parse_openrouter_chat_chunk(payload: &str) -> Result<Vec<ProviderStreamEvent>, ProviderError> {
    let mut accumulator = OpenRouterToolCallAccumulator::default();
    parse_openrouter_chat_chunk_with_accumulator(payload, &mut accumulator)
}

fn parse_openrouter_chat_chunk_with_accumulator(
    payload: &str,
    accumulator: &mut OpenRouterToolCallAccumulator,
) -> Result<Vec<ProviderStreamEvent>, ProviderError> {
    let chunk: OpenRouterChatChunk =
        serde_json::from_str(payload).map_err(|error| ProviderError::ResponseParsingFailed {
            provider_name: "OpenRouter".to_owned(),
            reason: error.to_string(),
        })?;
    let mut events = Vec::new();
    let usage = chunk.usage;

    if let Some(error) = chunk.error {
        return Err(openrouter_stream_error(error, payload));
    }

    if chunk.choices.is_empty() {
        let Some(usage) = usage else {
            return Err(ProviderError::MalformedResponse {
                provider_name: "OpenRouter".to_owned(),
                reason: format!(
                    "stream chunk omitted choices; OpenRouter response chunk JSON: {payload}"
                ),
            });
        };

        events.push(ProviderStreamEvent::Finished(ProviderFinished {
            finish_reason: FinishReason::Stop,
            tool_calls: Vec::new(),
            usage: Some(usage),
            reasoning: None,
        }));
        return Ok(events);
    }

    for choice in chunk.choices {
        let mut finish_reason = choice.finish_reason;
        let native_finish_reason = choice.native_finish_reason;
        let mut complete_tool_calls = Vec::new();
        if let Some(delta) = choice.delta {
            if let Some(tool_calls) = delta.tool_calls {
                accumulator.add_chunks(tool_calls)?;
            }
            if let Some(content) = delta.content {
                if !content.is_empty() {
                    events.push(ProviderStreamEvent::MessageDelta(MessageDelta::assistant(
                        content,
                    )));
                }
            }
            if let Some(reasoning) = delta.reasoning {
                if !reasoning.is_empty() {
                    events.push(ProviderStreamEvent::ReasoningDelta(ReasoningDelta {
                        content: reasoning,
                        metadata: None,
                    }));
                }
            }
            if let Some(refusal) = delta.refusal {
                if !refusal.is_empty() {
                    events.push(ProviderStreamEvent::MessageDelta(MessageDelta::assistant(
                        refusal,
                    )));
                }
            }
            if finish_reason.is_none() {
                finish_reason = delta.finish_reason;
            }
        }

        if let Some(message) = choice.message {
            if let Some(content) = message.content {
                if !content.is_empty() {
                    events.push(ProviderStreamEvent::MessageDelta(MessageDelta::assistant(
                        content,
                    )));
                }
            }
            if let Some(reasoning) = message.reasoning {
                if !reasoning.is_empty() {
                    events.push(ProviderStreamEvent::ReasoningDelta(ReasoningDelta {
                        content: reasoning,
                        metadata: None,
                    }));
                }
            }
            if let Some(refusal) = message.refusal {
                if !refusal.is_empty() {
                    events.push(ProviderStreamEvent::MessageDelta(MessageDelta::assistant(
                        refusal,
                    )));
                }
            }
            if let Some(tool_calls) = message.tool_calls {
                complete_tool_calls = tool_calls
                    .into_iter()
                    .enumerate()
                    .map(|(index, tool_call)| tool_call.into_provider_tool_call(index))
                    .collect::<Result<Vec<_>, _>>()?;
            }
        }

        if let Some(finish_reason) = finish_reason {
            let finish_reason = parse_openrouter_finish_reason(&finish_reason);
            let tool_calls = if finish_reason == FinishReason::ToolCalls {
                let mut accumulated = if accumulator.has_pending() {
                    accumulator.finish_tool_calls()?
                } else {
                    Vec::new()
                };
                accumulated.extend(complete_tool_calls);
                if accumulated.is_empty() {
                    return Err(ProviderError::MalformedResponse {
                        provider_name: "OpenRouter".to_owned(),
                        reason: openrouter_empty_tool_call_finish_reason(
                            native_finish_reason.as_deref(),
                            payload,
                        ),
                    });
                }
                accumulated
            } else {
                if accumulator.has_pending() || !complete_tool_calls.is_empty() {
                    return Err(ProviderError::MalformedResponse {
                        provider_name: "OpenRouter".to_owned(),
                        reason: format!(
                            "tool-call chunks ended without tool-call finish; OpenRouter response chunk JSON: {payload}"
                        ),
                    });
                }
                Vec::new()
            };

            events.push(ProviderStreamEvent::Finished(ProviderFinished {
                finish_reason,
                tool_calls,
                usage,
                reasoning: None,
            }));
        }
    }

    Ok(events)
}

fn parse_openrouter_finish_reason(reason: &str) -> FinishReason {
    match reason {
        "length" => FinishReason::Length,
        "tool_calls" => FinishReason::ToolCalls,
        "cancelled" => FinishReason::Cancelled,
        "content_filter" => FinishReason::ContentFilter,
        "error" => FinishReason::Error,
        _ => FinishReason::Stop,
    }
}

fn openrouter_stream_error(error: OpenRouterStreamError, payload: &str) -> ProviderError {
    ProviderError::StreamError {
        provider_name: "OpenRouter".to_owned(),
        code: error
            .code
            .as_ref()
            .and_then(openrouter_error_code_to_string),
        message: format!(
            "{}; OpenRouter response chunk JSON: {payload}",
            error.message
        ),
    }
}

fn openrouter_error_code_to_string(code: &serde_json::Value) -> Option<String> {
    match code {
        serde_json::Value::Null => None,
        serde_json::Value::String(code) if code.trim().is_empty() => None,
        serde_json::Value::String(code) => Some(code.clone()),
        serde_json::Value::Number(code) => Some(code.to_string()),
        _ => Some(code.to_string()),
    }
}

fn openrouter_empty_tool_call_finish_reason(
    native_finish_reason: Option<&str>,
    payload: &str,
) -> String {
    let native_finish_reason = native_finish_reason
        .filter(|reason| !reason.trim().is_empty())
        .unwrap_or("unknown");

    format!(
        "OpenRouter reported finish_reason=tool_calls without any tool call data \
         (no delta.tool_calls and no message.tool_calls). \
         native_finish_reason={native_finish_reason}. \
         This usually means the selected model/provider route stopped without emitting a native function call, \
         even though tools were present. Try a different tool-capable model/provider route or disable tools for this model. \
         OpenRouter response chunk JSON: {payload}"
    )
}

#[derive(Default)]
struct OpenRouterToolCallAccumulator {
    tool_calls: BTreeMap<usize, OpenRouterAccumulatedToolCall>,
}

impl OpenRouterToolCallAccumulator {
    fn add_chunks(
        &mut self,
        tool_calls: Vec<OpenRouterChatDeltaToolCall>,
    ) -> Result<(), ProviderError> {
        for tool_call in tool_calls {
            if let Some(kind) = tool_call.kind.as_deref() {
                if kind != "function" {
                    return Err(ProviderError::MalformedResponse {
                        provider_name: "OpenRouter".to_owned(),
                        reason: format!("unsupported tool-call type `{kind}`"),
                    });
                }
            }

            let accumulated = self.tool_calls.entry(tool_call.index).or_default();
            if let Some(id) = tool_call.id {
                if let Some(existing_id) = accumulated.id.as_deref() {
                    if existing_id != id {
                        return Err(ProviderError::MalformedResponse {
                            provider_name: "OpenRouter".to_owned(),
                            reason: format!(
                                "tool-call index {} changed id from `{existing_id}` to `{id}`",
                                tool_call.index
                            ),
                        });
                    }
                } else {
                    accumulated.id = Some(id);
                }
            }

            if let Some(function) = tool_call.function {
                if let Some(name) = function.name {
                    accumulated.name.push_str(&name);
                }
                if let Some(arguments) = function.arguments {
                    accumulated.arguments.push_str(&arguments);
                    accumulated.saw_arguments = true;
                }
            }
        }

        Ok(())
    }

    fn has_pending(&self) -> bool {
        !self.tool_calls.is_empty()
    }

    fn finish_tool_calls(&mut self) -> Result<Vec<ProviderToolCall>, ProviderError> {
        if self.tool_calls.is_empty() {
            return Err(ProviderError::MalformedResponse {
                provider_name: "OpenRouter".to_owned(),
                reason: "tool-call finish did not include tool-call chunks".to_owned(),
            });
        }

        let tool_calls = std::mem::take(&mut self.tool_calls);
        tool_calls
            .into_iter()
            .map(|(index, accumulated)| accumulated.into_provider_tool_call(index))
            .collect()
    }
}

#[derive(Default)]
struct OpenRouterAccumulatedToolCall {
    id: Option<String>,
    name: String,
    arguments: String,
    saw_arguments: bool,
}

impl OpenRouterAccumulatedToolCall {
    fn into_provider_tool_call(self, index: usize) -> Result<ProviderToolCall, ProviderError> {
        let Some(id) = self.id.filter(|id| !id.trim().is_empty()) else {
            return Err(ProviderError::MalformedResponse {
                provider_name: "OpenRouter".to_owned(),
                reason: format!("tool-call index {index} omitted id"),
            });
        };

        if self.name.trim().is_empty() {
            return Err(ProviderError::MalformedResponse {
                provider_name: "OpenRouter".to_owned(),
                reason: format!("tool-call index {index} omitted function name"),
            });
        }

        if !self.saw_arguments {
            return Err(ProviderError::MalformedResponse {
                provider_name: "OpenRouter".to_owned(),
                reason: format!("tool-call index {index} omitted function arguments"),
            });
        }

        Ok(ProviderToolCall::new(id, self.name, self.arguments))
    }
}

pub(crate) fn validate_openrouter_api_key(
    api_key: &str,
    request_current_key: impl FnOnce(&str) -> Result<u16, ProviderError>,
) -> Result<(), ProviderError> {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err(ProviderError::InvalidApiKey);
    }

    match request_current_key(api_key)? {
        200 => Ok(()),
        401 | 403 => Err(ProviderError::InvalidApiKey),
        _ => Err(ProviderError::ProviderUnavailable {
            provider_name: "OpenRouter".to_owned(),
        }),
    }
}

pub(crate) fn fetch_openrouter_models(
    api_key: &str,
    request_models: impl FnOnce(&str) -> Result<(u16, String), ProviderError>,
) -> Result<Vec<Model>, ProviderError> {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err(ProviderError::InvalidApiKey);
    }

    let (status, body) = request_models(api_key)?;
    match status {
        200 => parse_openrouter_models(&body),
        401 | 403 => Err(ProviderError::InvalidApiKey),
        _ => Err(ProviderError::ModelFetchFailed {
            provider_name: "OpenRouter".to_owned(),
        }),
    }
}

fn parse_openrouter_models(body: &str) -> Result<Vec<Model>, ProviderError> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|error| ProviderError::ResponseParsingFailed {
            provider_name: "OpenRouter".to_owned(),
            reason: error.to_string(),
        })?;
    let response: OpenRouterModelsResponse =
        serde_json::from_value(value).map_err(|error| ProviderError::MalformedResponse {
            provider_name: "OpenRouter".to_owned(),
            reason: error.to_string(),
        })?;
    let models = response
        .data
        .into_iter()
        .filter_map(|model| {
            let id = model.id.trim();
            if id.is_empty() {
                return None;
            }

            let display_name = model
                .name
                .as_deref()
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .unwrap_or(id);

            Some(Model::new(id, display_name))
        })
        .collect::<Vec<_>>();

    if models.is_empty() {
        return Err(ProviderError::NoModelsReturned {
            provider_name: "OpenRouter".to_owned(),
        });
    }

    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::super::dto::OpenRouterChatRequest;
    use super::super::sse::OpenRouterSseParser;
    use super::super::OpenRouterProvider;
    use super::*;
    use crate::{
        LlmProvider, ProviderCapabilities, ProviderContextLimits, ProviderMessage,
        ProviderMessageRole, ToolManifest, UsageMetadata,
    };
    use serde_json::json;

    #[test]
    fn openrouter_validation_rejects_blank_key_without_http_call() {
        let error =
            validate_openrouter_api_key("   ", |_| panic!("HTTP must not be called")).unwrap_err();

        assert!(matches!(error, ProviderError::InvalidApiKey));
    }

    #[test]
    fn openrouter_validation_accepts_success_status() {
        let result = validate_openrouter_api_key("sk-or-v1-valid", |_| Ok(200));

        assert!(result.is_ok());
    }

    #[test]
    fn openrouter_validation_rejects_unauthorized_status() {
        let error = validate_openrouter_api_key("sk-or-v1-invalid", |_| Ok(401)).unwrap_err();

        assert!(matches!(error, ProviderError::InvalidApiKey));
    }

    #[test]
    fn openrouter_validation_maps_other_statuses_to_unavailable() {
        let error = validate_openrouter_api_key("sk-or-v1-valid", |_| Ok(500)).unwrap_err();

        assert!(matches!(error, ProviderError::ProviderUnavailable { .. }));
    }

    #[test]
    fn openrouter_model_fetch_rejects_blank_key_without_http_call() {
        let error =
            fetch_openrouter_models("   ", |_| panic!("HTTP must not be called")).unwrap_err();

        assert!(matches!(error, ProviderError::InvalidApiKey));
    }

    #[test]
    fn openrouter_model_fetch_parses_model_ids_and_names() {
        let models = fetch_openrouter_models("sk-or-v1-valid", |_| {
            Ok((
                200,
                r#"{
                    "data": [
                        {"id": "openai/gpt-4o", "name": "GPT-4o"},
                        {"id": "anthropic/claude-sonnet-4.5", "name": null}
                    ]
                }"#
                .to_owned(),
            ))
        })
        .unwrap();

        assert_eq!(models[0], Model::new("openai/gpt-4o", "GPT-4o"));
        assert_eq!(
            models[1],
            Model::new("anthropic/claude-sonnet-4.5", "anthropic/claude-sonnet-4.5")
        );
    }

    #[test]
    fn openrouter_model_fetch_rejects_unauthorized_status() {
        let error = fetch_openrouter_models("sk-or-v1-invalid", |_| Ok((401, "{}".to_owned())))
            .unwrap_err();

        assert!(matches!(error, ProviderError::InvalidApiKey));
    }

    #[test]
    fn openrouter_model_fetch_rejects_empty_model_list() {
        let error =
            fetch_openrouter_models("sk-or-v1-valid", |_| Ok((200, r#"{"data":[]}"#.to_owned())))
                .unwrap_err();

        assert!(matches!(error, ProviderError::NoModelsReturned { .. }));
    }

    #[test]
    fn openrouter_model_fetch_maps_invalid_json_to_parse_error() {
        let error = fetch_openrouter_models("sk-or-v1-valid", |_| Ok((200, "not json".to_owned())))
            .unwrap_err();

        assert!(matches!(error, ProviderError::ResponseParsingFailed { .. }));
    }

    #[test]
    fn openrouter_model_fetch_maps_bad_shape_to_malformed_response() {
        let error = fetch_openrouter_models("sk-or-v1-valid", |_| {
            Ok((200, r#"{"data":[{"id":5,"name":"bad"}]}"#.to_owned()))
        })
        .unwrap_err();

        assert!(matches!(error, ProviderError::MalformedResponse { .. }));
    }

    #[test]
    fn openrouter_exposes_async_stream_seam() {
        let provider = OpenRouterProvider::new(String::new());
        let request =
            ProviderRequest::new(vec![ProviderMessage::user("hello")]).with_model("test/model");
        let error = match futures::executor::block_on(
            provider.stream_completion(request, Cancellation::default()),
        ) {
            Ok(_) => panic!("stream should require an API key"),
            Err(error) => error,
        };

        assert!(matches!(error, ProviderError::InvalidApiKey));
    }

    #[test]
    fn openrouter_provider_drops_inside_async_context_without_nested_runtime_panic() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async {
            let provider = OpenRouterProvider::new(String::new());
            drop(provider);
        });
    }

    #[test]
    fn openrouter_chat_chunk_parses_content_and_finish() {
        let events = parse_openrouter_chat_chunk(
            r#"{"choices":[{"delta":{"content":"hello"},"finish_reason":null},{"delta":{},"finish_reason":"stop"}]}"#,
        )
        .unwrap();

        assert!(matches!(
            &events[0],
            ProviderStreamEvent::MessageDelta(MessageDelta { content, .. }) if content == "hello"
        ));
        assert!(matches!(
            events[1],
            ProviderStreamEvent::Finished(ProviderFinished {
                finish_reason: FinishReason::Stop,
                ..
            })
        ));
    }

    #[test]
    fn openrouter_chat_request_maps_model_and_messages() {
        let request = OpenRouterChatRequest::from_provider_request(
            ProviderRequest::new(vec![
                ProviderMessage::system("system"),
                ProviderMessage::user("hello"),
                ProviderMessage::assistant("hi"),
                ProviderMessage::tool("tool output"),
            ])
            .with_model("openrouter/model"),
        )
        .unwrap();

        assert_eq!(request.model, "openrouter/model");
        assert!(request.stream);
        assert_eq!(request.messages[0].role, "system");
        assert_eq!(request.messages[1].role, "user");
        assert_eq!(request.messages[2].role, "assistant");
        assert_eq!(request.messages[3].role, "tool");
    }

    #[test]
    fn openrouter_chat_request_serializes_strict_tool_manifest() {
        let request = OpenRouterChatRequest::from_provider_request(
            ProviderRequest::new(vec![ProviderMessage::user("hello")])
                .with_model("openrouter/model")
                .with_tools(vec![ToolManifest::new(
                    "read_release",
                    "Read a release artifact.",
                    json!({
                        "type": "object",
                        "properties": {
                            "path": {"type": "string"}
                        },
                        "required": ["path"]
                    }),
                )]),
        )
        .unwrap();
        let value = serde_json::to_value(request).unwrap();

        assert_eq!(value["tools"][0]["type"], "function");
        assert_eq!(value["tools"][0]["function"]["name"], "read_release");
        assert_eq!(value["tools"][0]["function"]["strict"], true);
        assert_eq!(value["parallel_tool_calls"], false);
    }

    #[test]
    fn openrouter_chat_request_serializes_tool_call_and_tool_result_replay() {
        let request = OpenRouterChatRequest::from_provider_request(
            ProviderRequest::new(vec![
                ProviderMessage::assistant_tool_call(ProviderToolCall::new(
                    "call-1",
                    "read_release",
                    r#"{"path":"release/resume.yaml"}"#,
                )),
                ProviderMessage::tool_result("call-1", r#"{"ok":true}"#),
            ])
            .with_model("openrouter/model"),
        )
        .unwrap();
        let value = serde_json::to_value(request).unwrap();

        assert_eq!(value["messages"][0]["role"], "assistant");
        assert!(value["messages"][0].get("content").is_none());
        assert_eq!(value["messages"][0]["tool_calls"][0]["id"], "call-1");
        assert_eq!(value["messages"][0]["tool_calls"][0]["type"], "function");
        assert_eq!(
            value["messages"][0]["tool_calls"][0]["function"]["name"],
            "read_release"
        );
        assert_eq!(
            value["messages"][0]["tool_calls"][0]["function"]["arguments"],
            r#"{"path":"release/resume.yaml"}"#
        );
        assert_eq!(value["messages"][1]["role"], "tool");
        assert_eq!(value["messages"][1]["tool_call_id"], "call-1");
        assert_eq!(value["messages"][1]["content"], r#"{"ok":true}"#);
    }

    #[test]
    fn openrouter_chat_request_serializes_reasoning_effort() {
        let mut provider_request =
            ProviderRequest::new(vec![ProviderMessage::user("hello")]).with_model("test/model");
        provider_request.flags.include_reasoning = true;
        provider_request.flags.reasoning_effort = Some("high".to_owned());

        let request = OpenRouterChatRequest::from_provider_request(provider_request).unwrap();
        let value = serde_json::to_value(request).unwrap();

        assert_eq!(value["reasoning"]["effort"], "high");
    }

    #[test]
    fn openrouter_chat_chunks_emit_tool_calls_only_on_tool_call_finish() {
        let mut accumulator = OpenRouterToolCallAccumulator::default();

        let events = parse_openrouter_chat_chunk_with_accumulator(
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call-1","type":"function","function":{"name":"read_release","arguments":"{\"path\""}}]},"finish_reason":null}]}"#,
            &mut accumulator,
        )
        .unwrap();
        assert!(events.is_empty());

        let events = parse_openrouter_chat_chunk_with_accumulator(
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":":\"release/resume.yaml\"}"}}]},"finish_reason":null}]}"#,
            &mut accumulator,
        )
        .unwrap();
        assert!(events.is_empty());

        let events = parse_openrouter_chat_chunk_with_accumulator(
            r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#,
            &mut accumulator,
        )
        .unwrap();

        assert_eq!(
            events,
            vec![ProviderStreamEvent::Finished(ProviderFinished {
                finish_reason: FinishReason::ToolCalls,
                tool_calls: vec![ProviderToolCall::new(
                    "call-1",
                    "read_release",
                    r#"{"path":"release/resume.yaml"}"#
                )],
                usage: None,
                reasoning: None,
            })]
        );
    }

    #[test]
    fn openrouter_stream_stops_after_valid_tool_call_finish_before_duplicate_empty_finish() {
        let mut state = OpenRouterStreamState::default();
        let (sender, mut receiver) = mpsc::channel(8);

        let should_stop = futures::executor::block_on(send_openrouter_payload_events(
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"tool_terminal_TXIsWhOkok7u4ZqvpAeG","type":"function","function":{"name":"terminal","arguments":"{\"command\":\"pwd\"}"}}]},"finish_reason":"tool_calls"}]}"#,
            &mut state,
            &sender,
        ))
        .unwrap();

        assert!(should_stop);
        let event = receiver.try_recv().unwrap().unwrap();
        assert_eq!(
            event,
            ProviderStreamEvent::Finished(ProviderFinished {
                finish_reason: FinishReason::ToolCalls,
                tool_calls: vec![ProviderToolCall::new(
                    "tool_terminal_TXIsWhOkok7u4ZqvpAeG",
                    "terminal",
                    r#"{"command":"pwd"}"#
                )],
                usage: None,
                reasoning: None,
            })
        );
        assert!(receiver.try_recv().is_err());

        let duplicate_empty_finish = r#"{"choices":[{"delta":{"content":"","role":"assistant"},"finish_reason":"tool_calls","native_finish_reason":"STOP"}],"usage":{"prompt_tokens":814,"completion_tokens":130,"total_tokens":944}}"#;
        if !should_stop {
            futures::executor::block_on(send_openrouter_payload_events(
                duplicate_empty_finish,
                &mut state,
                &sender,
            ))
            .unwrap();
        }
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn openrouter_stream_waits_for_usage_only_chunk_after_text_finish() {
        let mut state = OpenRouterStreamState::default();
        let (sender, mut receiver) = mpsc::channel(8);

        let should_stop = futures::executor::block_on(send_openrouter_payload_events(
            r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#,
            &mut state,
            &sender,
        ))
        .unwrap();

        assert!(!should_stop);
        assert!(receiver.try_recv().is_err());

        let should_stop = futures::executor::block_on(send_openrouter_payload_events(
            r#"{"choices":[],"usage":{"prompt_tokens":12,"completion_tokens":34,"total_tokens":46}}"#,
            &mut state,
            &sender,
        ))
        .unwrap();

        assert!(should_stop);
        assert_eq!(
            receiver.try_recv().unwrap().unwrap(),
            ProviderStreamEvent::Finished(ProviderFinished {
                finish_reason: FinishReason::Stop,
                tool_calls: Vec::new(),
                usage: Some(UsageMetadata {
                    input_tokens: Some(12),
                    output_tokens: Some(34),
                    total_tokens: Some(46),
                }),
                reasoning: None,
            })
        );
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn openrouter_sse_parser_accepts_sdk_event_stream_shapes() {
        let mut parser = OpenRouterSseParser::default();
        let payloads = parser
            .push(
                b": keep-alive\r\nevent: message\r\ndata:{\"choices\":[{\"delta\":{\"content\":\"hi\"},\"finish_reason\":null}]}\r\n\r\n",
            )
            .unwrap();

        assert_eq!(
            payloads,
            vec![r#"{"choices":[{"delta":{"content":"hi"},"finish_reason":null}]}"#.to_owned()]
        );

        let mut parser = OpenRouterSseParser::default();
        let payloads = parser
            .push(
                b"data: {\"choices\":[\ndata: {\"delta\":{\"content\":\"there\"},\"finish_reason\":null}]}\n\n",
            )
            .unwrap();
        let events = parse_openrouter_chat_chunk(&payloads[0]).unwrap();

        assert_eq!(
            events,
            vec![ProviderStreamEvent::MessageDelta(MessageDelta::assistant(
                "there"
            ))]
        );
    }

    #[test]
    fn openrouter_chat_chunk_accepts_complete_message_tool_calls_on_tool_call_finish() {
        let events = parse_openrouter_chat_chunk(
            r#"{"choices":[{"message":{"role":"assistant","content":null,"tool_calls":[{"id":"call-1","type":"function","function":{"name":"read_release","arguments":"{\"path\":\"release/resume.yaml\"}"}}]},"finish_reason":"tool_calls"}]}"#,
        )
        .unwrap();

        assert_eq!(
            events,
            vec![ProviderStreamEvent::Finished(ProviderFinished {
                finish_reason: FinishReason::ToolCalls,
                tool_calls: vec![ProviderToolCall::new(
                    "call-1",
                    "read_release",
                    r#"{"path":"release/resume.yaml"}"#
                )],
                usage: None,
                reasoning: None,
            })]
        );
    }

    #[test]
    fn openrouter_chat_chunk_accepts_delta_finish_reason_from_tool_call_guide_shape() {
        let events = parse_openrouter_chat_chunk(
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call-1","type":"function","function":{"name":"read_release","arguments":"{\"path\":\"release/resume.yaml\"}"}}],"finish_reason":"tool_calls"}}]}"#,
        )
        .unwrap();

        assert_eq!(
            events,
            vec![ProviderStreamEvent::Finished(ProviderFinished {
                finish_reason: FinishReason::ToolCalls,
                tool_calls: vec![ProviderToolCall::new(
                    "call-1",
                    "read_release",
                    r#"{"path":"release/resume.yaml"}"#
                )],
                usage: None,
                reasoning: None,
            })]
        );
    }

    #[test]
    fn openrouter_tool_call_finish_without_tool_data_reports_raw_response_chunk() {
        let payload = r#"{"id":"gen-1777903368-VISjaqh4vj28SScWcgcH","object":"chat.completion.chunk","created":1777903368,"model":"google/gemini-3.1-pro-preview-20260219","provider":"Google","choices":[{"index":0,"delta":{"content":"","role":"assistant"},"finish_reason":"tool_calls","native_finish_reason":"STOP"}],"usage":{"prompt_tokens":818,"completion_tokens":260,"total_tokens":1078,"cost":0.004756,"is_byok":false,"prompt_tokens_details":{"cached_tokens":0,"cache_write_tokens":0,"audio_tokens":0,"video_tokens":0},"cost_details":{"upstream_inference_cost":0.004756,"upstream_inference_prompt_cost":0.001636,"upstream_inference_completions_cost":0.00312},"completion_tokens_details":{"reasoning_tokens":228,"image_tokens":0,"audio_tokens":0}}}"#;

        let error = parse_openrouter_chat_chunk(payload).unwrap_err();

        assert!(matches!(error, ProviderError::MalformedResponse { .. }));
        let message = error.to_string();
        assert!(message.contains("finish_reason=tool_calls without any tool call data"));
        assert!(message.contains("native_finish_reason=STOP"));
        assert!(message.contains(
            "selected model/provider route stopped without emitting a native function call"
        ));
        assert!(message.contains(payload));
    }

    #[test]
    fn openrouter_chat_chunk_parses_usage_only_terminal_chunk() {
        let events = parse_openrouter_chat_chunk(
            r#"{"choices":[],"usage":{"prompt_tokens":12,"completion_tokens":34,"total_tokens":46}}"#,
        )
        .unwrap();

        assert_eq!(
            events,
            vec![ProviderStreamEvent::Finished(ProviderFinished {
                finish_reason: FinishReason::Stop,
                tool_calls: Vec::new(),
                usage: Some(UsageMetadata {
                    input_tokens: Some(12),
                    output_tokens: Some(34),
                    total_tokens: Some(46),
                }),
                reasoning: None,
            })]
        );
    }

    #[test]
    fn openrouter_chat_chunk_parses_openrouter_usage_token_names() {
        let events = parse_openrouter_chat_chunk(
            r#"{"choices":[{"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":818,"completion_tokens":260,"total_tokens":1078}}"#,
        )
        .unwrap();

        assert_eq!(
            events,
            vec![ProviderStreamEvent::Finished(ProviderFinished {
                finish_reason: FinishReason::Stop,
                tool_calls: Vec::new(),
                usage: Some(UsageMetadata {
                    input_tokens: Some(818),
                    output_tokens: Some(260),
                    total_tokens: Some(1078),
                }),
                reasoning: None,
            })]
        );
    }

    #[test]
    fn openrouter_chat_chunk_parses_reasoning_and_refusal_deltas() {
        let events = parse_openrouter_chat_chunk(
            r#"{"choices":[{"delta":{"reasoning":"thinking","refusal":"I can't help with that."},"finish_reason":null}]}"#,
        )
        .unwrap();

        assert_eq!(
            events,
            vec![
                ProviderStreamEvent::ReasoningDelta(ReasoningDelta {
                    content: "thinking".to_owned(),
                    metadata: None,
                }),
                ProviderStreamEvent::MessageDelta(MessageDelta::assistant(
                    "I can't help with that."
                )),
            ]
        );
    }

    #[test]
    fn openrouter_chat_chunk_maps_content_filter_finish_reason() {
        let events = parse_openrouter_chat_chunk(
            r#"{"choices":[{"delta":{},"finish_reason":"content_filter"}]}"#,
        )
        .unwrap();

        assert_eq!(
            events,
            vec![ProviderStreamEvent::Finished(ProviderFinished {
                finish_reason: FinishReason::ContentFilter,
                tool_calls: Vec::new(),
                usage: None,
                reasoning: None,
            })]
        );
    }

    #[test]
    fn openrouter_chat_chunk_reports_top_level_stream_errors() {
        let payload = r#"{"error":{"code":429,"message":"rate limited"}}"#;
        let error = parse_openrouter_chat_chunk(payload).unwrap_err();

        assert!(matches!(error, ProviderError::StreamError { .. }));
        let message = error.to_string();
        assert!(message.contains("429"));
        assert!(message.contains("rate limited"));
        assert!(message.contains(payload));
    }

    #[test]
    fn malformed_openrouter_chat_chunk_returns_provider_error() {
        let error = parse_openrouter_chat_chunk("{not json").unwrap_err();

        assert!(matches!(error, ProviderError::ResponseParsingFailed { .. }));
    }

    #[test]
    fn provider_request_defaults_to_no_tool_streaming_call() {
        let request = ProviderRequest::new(vec![ProviderMessage::user("hello")]);

        assert_eq!(request.messages[0].role, ProviderMessageRole::User);
        assert!(request.tools.is_empty());
        assert!(request.flags.stream);
        assert!(!request.flags.allow_tools);
        assert!(!request.flags.include_reasoning);
        assert_eq!(request.flags.reasoning_effort, None);
    }

    #[test]
    fn provider_messages_support_tool_role() {
        let message = ProviderMessage::tool("tool output");

        assert_eq!(message.role, ProviderMessageRole::Tool);
        assert_eq!(message.content, "tool output");
        assert!(message.tool_calls.is_empty());
        assert_eq!(message.tool_call_id, None);
    }

    #[test]
    fn provider_capabilities_default_to_no_optional_features_or_limits() {
        let capabilities = ProviderCapabilities::default();

        assert!(!capabilities.streaming);
        assert!(!capabilities.tool_calls);
        assert!(!capabilities.structured_output);
        assert!(!capabilities.usage_metadata);
        assert!(!capabilities.reasoning_metadata);
        assert_eq!(
            capabilities.context_limits,
            ProviderContextLimits::default()
        );
    }

    #[test]
    fn openrouter_capabilities_are_available_without_network() {
        let capabilities = OpenRouterProvider::new(String::new()).capabilities();

        assert!(capabilities.streaming);
        assert!(capabilities.tool_calls);
        assert!(capabilities.reasoning);
        assert!(capabilities.usage_metadata);
    }
}
