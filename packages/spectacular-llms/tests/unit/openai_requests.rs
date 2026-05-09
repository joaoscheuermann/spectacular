#[test]
fn openai_responses_request_maps_messages_and_instructions() {
    let request = OpenAiResponsesRequest::from_provider_request(
        ProviderRequest::new(vec![
            ProviderMessage::system("system"),
            ProviderMessage::user("hello"),
            ProviderMessage::assistant("hi"),
        ])
        .with_model("gpt-5.5"),
    )
    .unwrap();
    let value = serde_json::to_value(request).unwrap();

    assert_eq!(value["model"], "gpt-5.5");
    assert_eq!(value["store"], false);
    assert_eq!(value["instructions"], "system");
    assert_eq!(value["input"][0]["role"], "user");
    assert_eq!(value["input"][0]["content"][0]["type"], "input_text");
    assert_eq!(value["input"][1]["role"], "assistant");
    assert_eq!(value["input"][1]["content"][0]["type"], "output_text");
}

#[test]
fn openai_responses_request_serializes_tools_for_responses_api() {
    let request = OpenAiResponsesRequest::from_provider_request(
        ProviderRequest::new(vec![ProviderMessage::user("hello")])
            .with_model("gpt-5.5")
            .with_tools(vec![ToolManifest::new(
                "terminal",
                "Run a command.",
                json!({
                    "type": "object",
                    "properties": {"command": {"type": "string"}},
                    "required": ["command"]
                }),
            )]),
    )
    .unwrap();
    let value = serde_json::to_value(request).unwrap();

    assert_eq!(value["tools"][0]["type"], "function");
    assert_eq!(value["tools"][0]["name"], "terminal");
    assert_eq!(value["tools"][0]["strict"], false);
    assert_eq!(value["tool_choice"], "auto");
    assert_eq!(value["parallel_tool_calls"], false);
}

#[test]
fn openai_responses_request_replays_function_call_and_output() {
    let request = OpenAiResponsesRequest::from_provider_request(
        ProviderRequest::new(vec![
            ProviderMessage::assistant_tool_call(ProviderToolCall::new(
                "call-1",
                "terminal",
                r#"{"command":"pwd"}"#,
            )),
            ProviderMessage::tool_result("call-1", r#"{"exit_code":0}"#),
        ])
        .with_model("gpt-5.5"),
    )
    .unwrap();
    let value = serde_json::to_value(request).unwrap();

    assert_eq!(value["input"][0]["type"], "function_call");
    assert_eq!(value["input"][0]["call_id"], "call-1");
    assert_eq!(value["input"][0]["name"], "terminal");
    assert_eq!(value["input"][1]["type"], "function_call_output");
    assert_eq!(value["input"][1]["call_id"], "call-1");
}

#[test]
fn openai_responses_request_serializes_reasoning_effort_and_summary() {
    let mut request =
        ProviderRequest::new(vec![ProviderMessage::user("hello")]).with_model("gpt-5.5");
    request.flags.reasoning_effort = Some("high".to_owned());

    let request = OpenAiResponsesRequest::from_provider_request(request).unwrap();
    let value = serde_json::to_value(request).unwrap();

    assert_eq!(value["reasoning"]["effort"], "high");
    assert_eq!(value["reasoning"]["summary"], "auto");
}

#[test]
fn openai_responses_request_serializes_reasoning_summary_when_included_without_effort() {
    let mut request =
        ProviderRequest::new(vec![ProviderMessage::user("hello")]).with_model("gpt-5.5");
    request.flags.include_reasoning = true;

    let request = OpenAiResponsesRequest::from_provider_request(request).unwrap();
    let value = serde_json::to_value(request).unwrap();

    assert!(value["reasoning"].get("effort").is_none());
    assert_eq!(value["reasoning"]["summary"], "auto");
}
