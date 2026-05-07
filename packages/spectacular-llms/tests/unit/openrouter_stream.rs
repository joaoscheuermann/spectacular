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
    assert!(message
        .contains("selected model/provider route stopped without emitting a native function call"));
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
            ProviderStreamEvent::MessageDelta(MessageDelta::assistant("I can't help with that.")),
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
