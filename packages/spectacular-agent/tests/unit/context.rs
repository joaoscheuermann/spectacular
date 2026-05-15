use super::assembler::{ContextAssembler, ContextAssembly, ContextAssemblyInput};
use super::diagnostics::ContextSection;
use super::token_count::ApproximateTokenCounter;
use crate::{
    context::{provider_messages_from_store, validate_context_limits},
    AgentEvent, ContextPolicy, ContextSummary, Store,
};
use spectacular_llms::{
    FinishReason, ProviderContextLimits, ProviderMessage, ProviderMessageRole, ReasoningMetadata,
    UsageMetadata,
};

#[test]
/// Verifies provider context filters stored events down to model-relevant roles.
fn provider_context_includes_only_model_relevant_roles() {
    let mut store = Store::default();
    store.append(AgentEvent::user_prompt("user prompt"));
    store.append(AgentEvent::message_delta("message-1", "assistant response"));
    store.append(AgentEvent::tool_call_start("call-1", "read", "{}"));
    store.append(AgentEvent::tool_call_finish("call-1", "read", r#"{"ok":true}"#));
    store.append(AgentEvent::command_start(
        "cmd-1",
        "slash_command",
        "/git commit",
        "Git commit",
        "/git commit",
        Some("/repo".to_owned()),
    ));
    store.append(AgentEvent::command_delta(
        "cmd-1",
        "status",
        "staged diff loaded",
        1,
    ));
    store.append(AgentEvent::command_finished(
        "cmd-1",
        crate::CommandStatus::Success,
        "changes committed successfully",
    ));
    store.append(AgentEvent::reasoning_delta("reasoning-1", "private thought"));
    store.append(AgentEvent::UsageMetadata(UsageMetadata {
        input_tokens: Some(1),
        output_tokens: Some(2),
        total_tokens: Some(3),
    }));
    store.append(AgentEvent::ReasoningMetadata(ReasoningMetadata {
        effort: Some("low".to_owned()),
        summary: Some("private summary".to_owned()),
    }));
    store.append(AgentEvent::ContextTokenUsage(
        crate::ContextTokenUsage {
            input_tokens: 10,
            context_window_tokens: Some(100),
        },
    ));
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
/// Verifies streamed assistant deltas are coalesced into one provider message.
fn provider_context_coalesces_streamed_assistant_deltas() {
    let mut store = Store::default();
    store.append(AgentEvent::user_prompt("first prompt"));
    store.append(AgentEvent::message_delta("message-1", "first"));
    store.append(AgentEvent::reasoning_delta("reasoning-1", "hidden reasoning"));
    store.append(AgentEvent::message_delta("message-1", " response"));
    store.append(AgentEvent::Finished {
        finish_reason: FinishReason::Stop,
    });
    store.append(AgentEvent::user_prompt("second prompt"));
    store.append(AgentEvent::message_delta("message-2", "second response"));

    let messages = provider_messages_from_store("system prompt", &store);

    assert_eq!(
        messages
            .iter()
            .map(|message| (message.role, message.content.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (ProviderMessageRole::System, "system prompt"),
            (ProviderMessageRole::User, "first prompt"),
            (ProviderMessageRole::Assistant, "first response"),
            (ProviderMessageRole::User, "second prompt"),
            (ProviderMessageRole::Assistant, "second response"),
        ]
    );
}

#[test]
/// Verifies provider context limits allow messages within configured bounds.
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
/// Verifies provider context limits reject excessive message counts.
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
/// Verifies provider context limits reject excessive character counts.
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

#[test]
/// Verifies the default assembler policy preserves ordinary provider messages.
fn assembler_default_policy_preserves_provider_messages() {
    let mut store = Store::default();
    store.append(AgentEvent::user_prompt("user prompt"));
    store.append(AgentEvent::message_delta("message-1", "first"));
    store.append(AgentEvent::message_delta("message-1", " response"));
    store.append(AgentEvent::Finished {
        finish_reason: FinishReason::Stop,
    });
    let expected = provider_messages_from_store("system prompt", &store);

    let assembled = ContextAssembler::default()
        .assemble(ContextAssemblyInput {
            system_prompt: "system prompt".to_owned(),
            store: &store,
            provider_limits: ProviderContextLimits::default(),
            continuation_prompt: None,
        })
        .unwrap();

    let ContextAssembly::Ready(context) = assembled else {
        panic!("default policy should not request compaction");
    };
    assert_eq!(context.messages, expected);
}

#[test]
/// Verifies continuation prompts are appended without mutating stored events.
fn assembler_appends_continuation_prompt_without_mutating_store() {
    let mut store = Store::default();
    store.append(AgentEvent::user_prompt("prompt"));
    let original_events = store.events().to_vec();

    let assembled = ContextAssembler::default()
        .assemble(ContextAssemblyInput {
            system_prompt: "system".to_owned(),
            store: &store,
            provider_limits: ProviderContextLimits::default(),
            continuation_prompt: Some("continue"),
        })
        .unwrap();

    let ContextAssembly::Ready(context) = assembled else {
        panic!("continuation alone should not request compaction");
    };
    let continuation = context.messages.last().unwrap();
    assert_eq!(continuation.role, ProviderMessageRole::User);
    assert_eq!(continuation.content, "continue");
    assert_eq!(store.events(), original_events);
}

#[test]
/// Verifies explicit compaction thresholds request summaries for old turns.
fn explicit_auto_compaction_threshold_requests_summary_of_old_turns() {
    let mut store = Store::default();
    store.append(AgentEvent::user_prompt("old prompt"));
    store.append(AgentEvent::message_delta(
        "message-1",
        "old answer with enough text to exceed a tiny threshold",
    ));
    store.append(AgentEvent::Finished {
        finish_reason: FinishReason::Stop,
    });
    store.append(AgentEvent::user_prompt("current prompt"));
    let policy = ContextPolicy {
        auto_compact_at_tokens: Some(1),
        latest_turns_to_protect: 1,
        ..ContextPolicy::default()
    };

    let assembled = ContextAssembler::new(ApproximateTokenCounter, policy)
        .assemble(ContextAssemblyInput {
            system_prompt: "system".to_owned(),
            store: &store,
            provider_limits: ProviderContextLimits::default(),
            continuation_prompt: None,
        })
        .unwrap();

    let ContextAssembly::NeedsSummary(summary_request) = assembled else {
        panic!("explicit threshold should request summary compaction");
    };
    assert_eq!(summary_request.source_event_start, 0);
    assert_eq!(summary_request.source_event_end, 3);
    assert!(summary_request.transcript.contains("old prompt"));
    assert!(!summary_request.transcript.contains("current prompt"));
}

#[test]
/// Verifies command lifecycle records do not enter context-summary source text.
fn summary_requests_ignore_command_lifecycle_text() {
    let mut store = Store::default();
    store.append(AgentEvent::user_prompt("old prompt"));
    store.append(AgentEvent::command_start(
        "cmd-1",
        "slash_command",
        "/git commit",
        "Git commit",
        "/git commit secret-lifecycle",
        None,
    ));
    store.append(AgentEvent::command_delta(
        "cmd-1",
        "status",
        "generated commit message: secret-lifecycle",
        1,
    ));
    store.append(AgentEvent::command_finished(
        "cmd-1",
        crate::CommandStatus::Success,
        "changes committed successfully secret-lifecycle",
    ));
    store.append(AgentEvent::message_delta(
        "message-1",
        "old answer with enough text to exceed a tiny threshold",
    ));
    store.append(AgentEvent::Finished {
        finish_reason: FinishReason::Stop,
    });
    store.append(AgentEvent::user_prompt("current prompt"));
    let policy = ContextPolicy {
        auto_compact_at_tokens: Some(1),
        latest_turns_to_protect: 1,
        ..ContextPolicy::default()
    };

    let assembled = ContextAssembler::new(ApproximateTokenCounter, policy)
        .assemble(ContextAssemblyInput {
            system_prompt: "system".to_owned(),
            store: &store,
            provider_limits: ProviderContextLimits::default(),
            continuation_prompt: None,
        })
        .unwrap();

    let ContextAssembly::NeedsSummary(summary_request) = assembled else {
        panic!("explicit threshold should request summary compaction");
    };
    assert!(summary_request.transcript.contains("old prompt"));
    assert!(summary_request.transcript.contains("old answer"));
    assert!(!summary_request.transcript.contains("secret-lifecycle"));
    assert!(!summary_request.transcript.contains("changes committed successfully"));
    assert!(!summary_request.transcript.contains("current prompt"));
}

#[test]
/// Verifies model-window budgets derive hard thresholds that request summaries.
fn derived_hard_threshold_requests_summary_when_model_window_is_configured() {
    let mut store = Store::default();
    store.append(AgentEvent::user_prompt("old prompt"));
    store.append(AgentEvent::message_delta(
        "message-1",
        "old answer with enough text to exceed a tiny derived threshold",
    ));
    store.append(AgentEvent::Finished {
        finish_reason: FinishReason::Stop,
    });
    store.append(AgentEvent::user_prompt("current prompt"));
    let policy = ContextPolicy {
        model_context_window_tokens: Some(128),
        max_output_tokens: 0,
        safety_margin_tokens: 0,
        hard_compact_ratio_percent: 1,
        latest_turns_to_protect: 1,
        ..ContextPolicy::default()
    };

    let assembled = ContextAssembler::new(ApproximateTokenCounter, policy)
        .assemble(ContextAssemblyInput {
            system_prompt: "system".to_owned(),
            store: &store,
            provider_limits: ProviderContextLimits::default(),
            continuation_prompt: None,
        })
        .unwrap();

    assert!(matches!(assembled, ContextAssembly::NeedsSummary(_)));
}

#[test]
/// Verifies summary requests cap compacted source events to the summary budget.
fn summary_request_uses_budgeted_compactable_prefix() {
    let mut store = Store::default();
    store.append(AgentEvent::user_prompt("one"));
    store.append(AgentEvent::message_delta("message-1", "two"));
    store.append(AgentEvent::Finished {
        finish_reason: FinishReason::Stop,
    });
    store.append(AgentEvent::user_prompt(
        "second old prompt with much more text",
    ));
    store.append(AgentEvent::message_delta(
        "message-2",
        "second old response with enough text to exceed the summary source limit",
    ));
    store.append(AgentEvent::Finished {
        finish_reason: FinishReason::Stop,
    });
    store.append(AgentEvent::user_prompt("current prompt"));
    let policy = ContextPolicy {
        model_context_window_tokens: Some(32),
        max_output_tokens: 0,
        safety_margin_tokens: 0,
        hard_compact_ratio_percent: 1,
        summary_max_tokens: 0,
        summary_input_safety_margin_tokens: 20,
        latest_turns_to_protect: 1,
        ..ContextPolicy::default()
    };

    let assembled = ContextAssembler::new(ApproximateTokenCounter, policy)
        .assemble(ContextAssemblyInput {
            system_prompt: "system".to_owned(),
            store: &store,
            provider_limits: ProviderContextLimits::default(),
            continuation_prompt: None,
        })
        .unwrap();

    let ContextAssembly::NeedsSummary(summary_request) = assembled else {
        panic!("derived threshold should request summary compaction");
    };
    assert_eq!(summary_request.source_event_end, 3);
    assert!(summary_request.transcript.contains("one"));
    assert!(!summary_request.transcript.contains("second old prompt"));
}

#[test]
/// Verifies same-turn tool history can compact when there are no old user turns.
fn same_turn_tool_history_requests_summary_when_protected_turn_exceeds_threshold() {
    let mut store = Store::default();
    store.append(AgentEvent::user_prompt("current prompt"));
    store.append(AgentEvent::tool_call_start(
        "call-1",
        "read",
        r#"{"path":"one.txt"}"#,
    ));
    store.append(AgentEvent::tool_call_finish(
        "call-1",
        "read",
        "old tool output with enough content to cross a tiny threshold",
    ));
    store.append(AgentEvent::tool_call_start(
        "call-2",
        "read",
        r#"{"path":"two.txt"}"#,
    ));
    store.append(AgentEvent::tool_call_finish(
        "call-2",
        "read",
        "latest tool output should remain raw when possible",
    ));
    let policy = ContextPolicy {
        auto_compact_at_tokens: Some(1),
        latest_turns_to_protect: 6,
        ..ContextPolicy::default()
    };

    let assembled = ContextAssembler::new(ApproximateTokenCounter, policy)
        .assemble(ContextAssemblyInput {
            system_prompt: "system".to_owned(),
            store: &store,
            provider_limits: ProviderContextLimits::default(),
            continuation_prompt: None,
        })
        .unwrap();

    let ContextAssembly::NeedsSummary(summary_request) = assembled else {
        panic!("same-turn tool history should request summary compaction");
    };
    assert_eq!(summary_request.source_event_start, 0);
    assert_eq!(summary_request.source_event_end, 3);
    assert!(summary_request.transcript.contains("current prompt"));
    assert!(summary_request.transcript.contains("call-1"));
    assert!(summary_request.transcript.contains("one.txt"));
    assert!(!summary_request.transcript.contains("call-2"));
    assert!(!summary_request.transcript.contains("two.txt"));
}

#[test]
/// Verifies same-turn compaction does not split a pending tool call from its result.
fn same_turn_summary_stops_before_unmatched_tool_call() {
    let mut store = Store::default();
    store.append(AgentEvent::user_prompt("current prompt"));
    store.append(AgentEvent::tool_call_start(
        "call-1",
        "read",
        r#"{"path":"one.txt"}"#,
    ));
    store.append(AgentEvent::tool_call_finish(
        "call-1",
        "read",
        "old tool output with enough content to cross a tiny threshold",
    ));
    store.append(AgentEvent::tool_call_start(
        "call-2",
        "read",
        r#"{"path":"two.txt"}"#,
    ));
    let policy = ContextPolicy {
        auto_compact_at_tokens: Some(1),
        latest_turns_to_protect: 6,
        ..ContextPolicy::default()
    };

    let assembled = ContextAssembler::new(ApproximateTokenCounter, policy)
        .assemble(ContextAssemblyInput {
            system_prompt: "system".to_owned(),
            store: &store,
            provider_limits: ProviderContextLimits::default(),
            continuation_prompt: None,
        })
        .unwrap();

    let ContextAssembly::NeedsSummary(summary_request) = assembled else {
        panic!("same-turn compactable pair should request summary compaction");
    };
    assert_eq!(summary_request.source_event_end, 3);
    assert!(summary_request.transcript.contains("call-1"));
    assert!(!summary_request.transcript.contains("call-2"));
}

#[test]
/// Verifies soft thresholds only mark diagnostics without requesting compaction.
fn soft_threshold_only_records_diagnostics() {
    let mut store = Store::default();
    store.append(AgentEvent::user_prompt(
        "prompt with enough text for a tiny soft limit",
    ));
    let policy = ContextPolicy {
        soft_compact_at_tokens: Some(1),
        ..ContextPolicy::default()
    };

    let assembled = ContextAssembler::new(ApproximateTokenCounter, policy)
        .assemble(ContextAssemblyInput {
            system_prompt: "system".to_owned(),
            store: &store,
            provider_limits: ProviderContextLimits::default(),
            continuation_prompt: None,
        })
        .unwrap();

    let ContextAssembly::Ready(context) = assembled else {
        panic!("soft threshold should not request summary compaction");
    };
    assert!(context.diagnostics.soft_compaction_would_trigger);
    assert!(!context.diagnostics.compaction_would_trigger);
}

#[test]
/// Verifies stored summaries replace old raw turns in provider-visible context.
fn stored_summary_replaces_old_raw_turns_in_provider_context() {
    let store = Store::from(vec![
        AgentEvent::user_prompt("old prompt"),
        AgentEvent::message_delta("message-1", "old answer"),
        AgentEvent::ContextSummaryCreated(ContextSummary {
            id: "summary-1".to_owned(),
            replaces: None,
            source_event_start: 0,
            source_event_end: 2,
            content: "# Goal\nKeep the old facts compact.".to_owned(),
            estimated_tokens: 12,
        }),
        AgentEvent::user_prompt("current prompt"),
    ]);

    let assembled = ContextAssembler::default()
        .assemble(ContextAssemblyInput {
            system_prompt: "system".to_owned(),
            store: &store,
            provider_limits: ProviderContextLimits::default(),
            continuation_prompt: None,
        })
        .unwrap();

    let ContextAssembly::Ready(context) = assembled else {
        panic!("stored summary should assemble without a new compaction pass");
    };
    let text = context
        .messages
        .iter()
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("Compact session state"));
    assert!(text.contains("Keep the old facts compact"));
    assert!(text.contains("current prompt"));
    assert!(!text.contains("old prompt"));
    assert!(context
        .diagnostics
        .section_usage
        .iter()
        .any(|usage| usage.section == ContextSection::Summary));
}
