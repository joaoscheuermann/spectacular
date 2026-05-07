#[test]
fn openrouter_validation_rejects_blank_key_without_http_call() {
    let error = validate_openrouter_api_key("   ", |_| panic!("HTTP must not be called")).unwrap_err();

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
    let error = fetch_openrouter_models("   ", |_| panic!("HTTP must not be called")).unwrap_err();

    assert!(matches!(error, ProviderError::InvalidApiKey));
}

#[test]
fn openrouter_model_fetch_parses_model_ids_and_names() {
    let models = fetch_openrouter_models("sk-or-v1-valid", |_| {
            Ok((
                200,
                r#"{
                    "data": [
                        {"id": "openai/gpt-4o", "name": "GPT-4o", "supported_parameters": ["reasoning", "tools"]},
                        {"id": "anthropic/claude-sonnet-4.5", "name": null}
                    ]
                }"#
                .to_owned(),
            ))
        })
        .unwrap();

    assert_eq!(
        models[0],
        Model::with_supported_parameters(
            "openai/gpt-4o",
            "GPT-4o",
            ["reasoning".to_owned(), "tools".to_owned()]
        )
    );
    assert_eq!(
        models[1],
        Model::new("anthropic/claude-sonnet-4.5", "anthropic/claude-sonnet-4.5")
    );
    assert!(models[0].supports_parameter("reasoning"));
    assert!(!models[1].supports_parameter("reasoning"));
}

#[test]
fn openrouter_model_fetch_rejects_unauthorized_status() {
    let error =
        fetch_openrouter_models("sk-or-v1-invalid", |_| Ok((401, "{}".to_owned()))).unwrap_err();

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
