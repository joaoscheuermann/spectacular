use super::client::OpenRouterHttpClient;
use super::debug;
use super::dto::OpenRouterChatRequest;
#[cfg(test)]
use super::models::{fetch_openrouter_models, validate_openrouter_api_key};
#[cfg(test)]
use super::parser::parse_openrouter_chat_chunk;
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
    use super::super::sse::OpenRouterSseParser;
    use super::super::OpenRouterProvider;
    use super::*;
    use crate::{
        LlmDebugLogger, LlmProvider, MessageDelta, Model, ProviderCapabilities,
        ProviderContextLimits, ProviderMessage, ProviderMessageRole, ProviderToolCall,
        ReasoningDelta, ToolManifest, UsageMetadata,
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
            &LlmDebugLogger::disabled(),
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
                &LlmDebugLogger::disabled(),
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
            &LlmDebugLogger::disabled(),
            &sender,
        ))
        .unwrap();

        assert!(!should_stop);
        assert!(receiver.try_recv().is_err());

        let should_stop = futures::executor::block_on(send_openrouter_payload_events(
            r#"{"choices":[],"usage":{"prompt_tokens":12,"completion_tokens":34,"total_tokens":46}}"#,
            &mut state,
            &LlmDebugLogger::disabled(),
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
