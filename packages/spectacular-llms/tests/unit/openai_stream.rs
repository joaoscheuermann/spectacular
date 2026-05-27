#[test]
fn openai_stream_parses_text_delta() {
    let events =
        parse_openai_response_event(r#"{"type":"response.output_text.delta","delta":"hello"}"#)
            .unwrap();

    assert_eq!(
        events,
        vec![ProviderStreamEvent::MessageDelta(MessageDelta::assistant(
            "hello"
        ))]
    );
}

#[test]
fn openai_stream_parses_function_call_finish() {
    let events = parse_openai_response_event(
        r#"{"type":"response.output_item.done","item":{"type":"function_call","call_id":"call-1","name":"terminal","arguments":"{\"command\":\"pwd\"}"}}"#,
    )
    .unwrap();

    assert_eq!(
        events,
        vec![ProviderStreamEvent::Finished(ProviderFinished {
            finish_reason: FinishReason::ToolCalls,
            tool_calls: vec![ProviderToolCall::new(
                "call-1",
                "terminal",
                r#"{"command":"pwd"}"#
            )],
            usage: None,
            reasoning: None,
        })]
    );
}

#[test]
fn openai_stream_parses_completed_usage() {
    let events = parse_openai_response_event(
        r#"{"type":"response.completed","response":{"status":"completed","usage":{"input_tokens":12,"output_tokens":34,"total_tokens":46}}}"#,
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
fn openai_stream_parses_incomplete_as_length() {
    let events = parse_openai_response_event(
        r#"{"type":"response.incomplete","response":{"status":"incomplete","incomplete_details":{"reason":"max_output_tokens"}}}"#,
    )
    .unwrap();

    assert!(matches!(
        events.as_slice(),
        [ProviderStreamEvent::Finished(ProviderFinished {
            finish_reason: FinishReason::Length,
            ..
        })]
    ));
}

#[test]
fn openai_stream_invalid_json_exposes_bounded_payload_diagnostics() {
    let payload = format!("{{not json sk-secret_value}}\n\u{0007}{}", "x".repeat(1100));
    let error = parse_openai_response_event(&payload).unwrap_err();

    let ProviderError::ResponseParsingFailed {
        reason,
        diagnostics: Some(diagnostics),
        ..
    } = error
    else {
        panic!("expected response parse failure");
    };

    assert!(!reason.contains("{not json"));
    assert_eq!(diagnostics.stage, Some(ProviderErrorStage::PayloadParse));
    let excerpt = diagnostics.excerpt.unwrap();
    assert!(excerpt.starts_with("{not json "));
    assert!(!excerpt.contains("sk-secret_value"));
    assert!(excerpt.contains("[redacted]"));
    assert!(excerpt.len() <= 1024);
    assert!(!excerpt.chars().any(char::is_control));
    assert_eq!(
        diagnostics.debug_events,
        vec!["sse_payload".to_owned(), "payload_parse_error".to_owned()]
    );
}

#[test]
fn openai_stream_error_event_exposes_provider_code_and_payload_breadcrumb() {
    let payload =
        r#"{"type":"error","error":{"code":"rate_limit_exceeded","message":"rate limited"}}"#;
    let error = parse_openai_response_event(payload).unwrap_err();
    let rendered = error.to_string();

    let ProviderError::StreamError {
        code,
        message,
        diagnostics: Some(diagnostics),
        ..
    } = error
    else {
        panic!("expected stream error");
    };

    assert_eq!(code.as_deref(), Some("rate_limit_exceeded"));
    assert_eq!(message, "rate limited");
    assert!(!rendered.contains(payload));
    assert_eq!(diagnostics.stage, Some(ProviderErrorStage::ProviderStream));
    assert_eq!(
        diagnostics.provider_code.as_deref(),
        Some("rate_limit_exceeded")
    );
    assert_eq!(diagnostics.excerpt.as_deref(), Some(payload));
    assert_eq!(diagnostics.debug_events, vec!["sse_payload".to_owned()]);
}
