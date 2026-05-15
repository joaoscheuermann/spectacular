mod support;

use spectacular_agent::{Agent, AgentConfig, AgentError, AgentEvent, ContextPolicy, Store};
use spectacular_llms::{
    FinishReason, MessageDelta, ProviderContextLimits, ProviderFinished, ProviderStreamEvent,
    ProviderToolCall,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use support::{
    capabilities, finished_length_without_usage, finished_stop_with_usage, EchoTool, FakeProvider,
    RecordingProvider,
};

#[test]
/// Verifies malformed tool-call finishes are rejected and stored as errors.
fn malformed_tool_call_finish_is_rejected_and_stored() {
    let provider = FakeProvider {
        calls: Arc::new(AtomicUsize::new(0)),
        capabilities: capabilities(),
        events: vec![ProviderStreamEvent::Finished(ProviderFinished::tool_calls(
            vec![ProviderToolCall::new("call-1", "", "{}")],
        ))],
    };
    let mut agent = Agent::new(provider);
    agent.register_tool(EchoTool).unwrap();
    agent.enqueue_prompt("prompt");

    let error = futures::executor::block_on(agent.run_next()).unwrap_err();

    assert!(matches!(
        error,
        AgentError::MalformedProviderResponse { .. }
    ));
    assert!(matches!(
        agent.events().last(),
        Some(AgentEvent::Error { .. })
    ));
}

#[test]
/// Verifies a final response without required usage metadata is rejected.
fn missing_usage_metadata_on_final_response_is_rejected_and_stored() {
    let provider = FakeProvider {
        calls: Arc::new(AtomicUsize::new(0)),
        capabilities: capabilities(),
        events: vec![ProviderStreamEvent::Finished(ProviderFinished::stopped())],
    };
    let mut agent = Agent::new(provider);
    agent.enqueue_prompt("prompt");

    let error = futures::executor::block_on(agent.run_next()).unwrap_err();

    assert!(matches!(
        error,
        AgentError::MalformedProviderResponse { .. }
    ));
    assert!(matches!(
        agent.events().last(),
        Some(AgentEvent::Error { .. })
    ));
}

#[test]
/// Verifies context limit failures are stored before provider calls occur.
fn context_limit_failure_is_stored_before_provider_io() {
    let mut provider = FakeProvider::text("unused");
    provider.capabilities.context_limits = ProviderContextLimits {
        max_messages: Some(1),
        max_chars: None,
    };
    let calls = Arc::clone(&provider.calls);
    let mut agent = Agent::new(provider);
    agent.enqueue_prompt("prompt");

    let error = futures::executor::block_on(agent.run_next()).unwrap_err();

    assert!(matches!(error, AgentError::ContextLimitError { .. }));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
/// Verifies automatic compaction summarizes old context before the real request.
fn auto_compaction_summarizes_before_real_provider_request() {
    let provider = RecordingProvider::new(vec![
        vec![
            ProviderStreamEvent::MessageDelta(MessageDelta::assistant(
                "# Goal\nKeep prior work compact.",
            )),
            ProviderStreamEvent::Finished(finished_stop_with_usage()),
        ],
        vec![
            ProviderStreamEvent::MessageDelta(MessageDelta::assistant("done")),
            ProviderStreamEvent::Finished(finished_stop_with_usage()),
        ],
    ]);
    let calls = Arc::clone(&provider.calls);
    let requests = Arc::clone(&provider.requests);
    let store = Store::from(vec![
        AgentEvent::user_prompt("old prompt"),
        AgentEvent::message_start("message-old"),
        AgentEvent::message_delta(
            "message-old",
            "old answer with enough content to cross a tiny threshold",
        ),
        AgentEvent::message_finish("message-old"),
        AgentEvent::Finished {
            finish_reason: FinishReason::Stop,
        },
    ]);
    let agent = Agent::with_config_and_store(
        provider,
        AgentConfig {
            context_policy: ContextPolicy {
                auto_compact_at_tokens: Some(1),
                latest_turns_to_protect: 1,
                ..ContextPolicy::default()
            },
            ..AgentConfig::default()
        },
        store,
    );

    futures::executor::block_on(agent.run("current prompt")).unwrap();

    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert!(agent
        .events()
        .iter()
        .any(|event| matches!(event, AgentEvent::ContextSummaryCreated(_))));
    let requests = requests.lock().unwrap();
    assert_eq!(requests[0].tools.len(), 0);
    assert!(!requests[0].flags.allow_tools);
    assert!(requests[0].messages[0]
        .content
        .contains("Summarize compacted Spectacular session context"));

    let real_context = requests[1]
        .messages
        .iter()
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(real_context.contains("Compact session state"));
    assert!(real_context.contains("Keep prior work compact"));
    assert!(real_context.contains("current prompt"));
    assert!(!real_context.contains("old prompt"));
}

#[test]
/// Verifies automatic compaction can summarize same-turn tool output before the follow-up request.
fn auto_compaction_summarizes_same_turn_tool_output_before_follow_up_request() {
    let large_tool_output = "x".repeat(1600);
    let provider = RecordingProvider::new(vec![
        vec![ProviderStreamEvent::Finished(ProviderFinished::tool_calls(
            vec![ProviderToolCall::new(
                "call-1",
                "echo",
                format!(r#"{{"payload":"{large_tool_output}"}}"#),
            )],
        ))],
        vec![
            ProviderStreamEvent::MessageDelta(MessageDelta::assistant(
                "# Goal\nKeep current tool work compact.",
            )),
            ProviderStreamEvent::Finished(finished_stop_with_usage()),
        ],
        vec![
            ProviderStreamEvent::MessageDelta(MessageDelta::assistant("done")),
            ProviderStreamEvent::Finished(finished_stop_with_usage()),
        ],
    ]);
    let requests = Arc::clone(&provider.requests);
    let agent = Agent::with_config(
        provider,
        AgentConfig {
            context_policy: ContextPolicy {
                auto_compact_at_tokens: Some(100),
                latest_turns_to_protect: 6,
                ..ContextPolicy::default()
            },
            ..AgentConfig::default()
        },
    );
    agent.register_tool(EchoTool).unwrap();

    futures::executor::block_on(agent.run("current prompt")).unwrap();

    assert!(agent
        .events()
        .iter()
        .any(|event| matches!(event, AgentEvent::ContextSummaryCreated(_))));
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    assert!(requests[1].tools.is_empty());
    assert!(!requests[1].flags.allow_tools);

    let follow_up_context = requests[2]
        .messages
        .iter()
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(follow_up_context.contains("Compact session state"));
    assert!(follow_up_context.contains("Keep current tool work compact"));
    assert!(!follow_up_context.contains(&large_tool_output));
}

#[test]
/// Verifies failed hidden summaries prevent the real provider request.
fn summary_failure_prevents_real_provider_request() {
    let provider = RecordingProvider::new(vec![vec![ProviderStreamEvent::Finished(
        finished_length_without_usage(),
    )]]);
    let calls = Arc::clone(&provider.calls);
    let store = Store::from(vec![
        AgentEvent::user_prompt("old prompt"),
        AgentEvent::message_start("message-old"),
        AgentEvent::message_delta(
            "message-old",
            "old answer with enough content to cross a tiny threshold",
        ),
        AgentEvent::message_finish("message-old"),
        AgentEvent::Finished {
            finish_reason: FinishReason::Stop,
        },
    ]);
    let agent = Agent::with_config_and_store(
        provider,
        AgentConfig {
            context_policy: ContextPolicy {
                auto_compact_at_tokens: Some(1),
                latest_turns_to_protect: 1,
                ..ContextPolicy::default()
            },
            ..AgentConfig::default()
        },
        store,
    );

    let error = futures::executor::block_on(agent.run("current prompt")).unwrap_err();

    assert!(matches!(error, AgentError::ContextLimitError { .. }));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(!agent
        .events()
        .iter()
        .any(|event| matches!(event, AgentEvent::ContextSummaryCreated(_))));
    assert!(matches!(
        agent.events().last(),
        Some(AgentEvent::Error { .. })
    ));
}
