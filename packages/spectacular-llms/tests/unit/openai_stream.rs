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
