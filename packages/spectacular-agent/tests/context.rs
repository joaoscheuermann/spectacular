use spectacular_agent::{provider_messages_from_store, validate_context_limits, AgentEvent, Store};
use spectacular_llms::{
    FinishReason, MessageDelta, ProviderContextLimits, ProviderMessage, ProviderMessageRole,
    ReasoningDelta, ReasoningMetadata, UsageMetadata,
};

#[test]
fn provider_context_includes_only_model_relevant_roles() {
    let mut store = Store::default();
    store.append(AgentEvent::user_prompt("user prompt"));
    store.append(AgentEvent::MessageDelta(MessageDelta::assistant(
        "assistant response",
    )));
    store.append(AgentEvent::assistant_tool_call_request(
        "call-1", "read", "{}",
    ));
    store.append(AgentEvent::tool_result("call-1", "read", r#"{"ok":true}"#));
    store.append(AgentEvent::ReasoningDelta(ReasoningDelta {
        content: "private thought".to_owned(),
        metadata: Some(ReasoningMetadata::default()),
    }));
    store.append(AgentEvent::UsageMetadata(UsageMetadata {
        input_tokens: Some(1),
        output_tokens: Some(2),
        total_tokens: Some(3),
    }));
    store.append(AgentEvent::ReasoningMetadata(ReasoningMetadata {
        effort: Some("low".to_owned()),
        summary: Some("private summary".to_owned()),
    }));
    store.append(AgentEvent::validation_error("invalid json"));
    store.append(AgentEvent::error("provider failed"));
    store.append(AgentEvent::cancelled("cancelled"));
    store.append(AgentEvent::Finished {
        finish_reason: FinishReason::Stop,
    });
    store.append(AgentEvent::internal("queue bookkeeping"));

    let messages = provider_messages_from_store("system prompt", &store);

    assert_eq!(
        messages
            .iter()
            .map(|message| (message.role, message.content.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (ProviderMessageRole::System, "system prompt"),
            (ProviderMessageRole::User, "user prompt"),
            (ProviderMessageRole::Assistant, "assistant response"),
            (ProviderMessageRole::Assistant, ""),
            (ProviderMessageRole::Tool, r#"{"ok":true}"#),
        ]
    );
    assert_eq!(messages[3].tool_calls[0].id, "call-1");
    assert_eq!(messages[3].tool_calls[0].name, "read");
    assert_eq!(messages[3].tool_calls[0].arguments, "{}");
    assert_eq!(messages[4].tool_call_id.as_deref(), Some("call-1"));
}

#[test]
fn context_limits_allow_messages_within_bounds() {
    let messages = vec![
        ProviderMessage::system("system"),
        ProviderMessage::user("short"),
    ];

    assert!(validate_context_limits(
        &messages,
        ProviderContextLimits {
            max_messages: Some(2),
            max_chars: Some(11),
        },
    )
    .is_ok());
}

#[test]
fn context_limits_reject_too_many_messages() {
    let messages = vec![
        ProviderMessage::system("system"),
        ProviderMessage::user("user"),
    ];

    let error = validate_context_limits(
        &messages,
        ProviderContextLimits {
            max_messages: Some(1),
            max_chars: None,
        },
    )
    .unwrap_err();

    assert_eq!(error.to_string(), "2 messages exceeds limit 1");
}

#[test]
fn context_limits_reject_too_many_chars() {
    let messages = vec![
        ProviderMessage::system("system"),
        ProviderMessage::user("user"),
    ];

    let error = validate_context_limits(
        &messages,
        ProviderContextLimits {
            max_messages: None,
            max_chars: Some(5),
        },
    )
    .unwrap_err();

    assert_eq!(error.to_string(), "10 characters exceeds limit 5");
}
