mod support;

use spectacular_agent::{provider_messages_from_store, Agent, AgentEvent, Store};
use spectacular_llms::{
    FinishReason, MessageDelta, ProviderFinished, ProviderMessage, ProviderStreamEvent,
    ProviderToolCall, ReasoningDelta,
};
use support::{
    finished_stop_with_usage, EchoTool, FakeProvider, RecordingProvider, StreamErrorProvider,
};

#[test]
/// Verifies assistant provider deltas are wrapped in explicit lifecycle events with one stable ID.
fn assistant_stream_emits_explicit_message_lifecycle() {
    let mut agent = Agent::new(FakeProvider::with_events(vec![
        ProviderStreamEvent::MessageDelta(MessageDelta::assistant("hello ")),
        ProviderStreamEvent::MessageDelta(MessageDelta::assistant("world")),
        ProviderStreamEvent::Finished(finished_stop_with_usage()),
    ]));
    agent.enqueue_prompt("prompt");

    futures::executor::block_on(agent.run_next()).unwrap();

    let lifecycle = agent
        .events()
        .into_iter()
        .filter_map(|event| match event {
            AgentEvent::MessageStart { id } => Some(("start", id.to_string(), String::new())),
            AgentEvent::MessageDelta { id, content } => Some(("delta", id.to_string(), content)),
            AgentEvent::MessageFinish { id } => Some(("finish", id.to_string(), String::new())),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(lifecycle.len(), 4);
    assert_eq!(lifecycle[0].0, "start");
    assert_eq!(lifecycle[1].2, "hello ");
    assert_eq!(lifecycle[2].2, "world");
    assert_eq!(lifecycle[3].0, "finish");
    assert!(lifecycle.iter().all(|event| event.1 == lifecycle[0].1));
}

#[test]
/// Verifies reasoning provider deltas are wrapped in explicit lifecycle events with one stable ID.
fn reasoning_stream_emits_explicit_reasoning_lifecycle() {
    let mut agent = Agent::new(FakeProvider::with_events(vec![
        ProviderStreamEvent::ReasoningDelta(ReasoningDelta {
            content: "think ".to_owned(),
            metadata: None,
        }),
        ProviderStreamEvent::ReasoningDelta(ReasoningDelta {
            content: "more".to_owned(),
            metadata: None,
        }),
        ProviderStreamEvent::Finished(finished_stop_with_usage()),
    ]));
    agent.enqueue_prompt("prompt");

    futures::executor::block_on(agent.run_next()).unwrap();

    let lifecycle = agent
        .events()
        .into_iter()
        .filter_map(|event| match event {
            AgentEvent::ReasoningStart { id } => Some(("start", id.to_string(), String::new())),
            AgentEvent::ReasoningDelta { id, content } => Some(("delta", id.to_string(), content)),
            AgentEvent::ReasoningFinish { id } => Some(("finish", id.to_string(), String::new())),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(lifecycle.len(), 4);
    assert_eq!(lifecycle[0].0, "start");
    assert_eq!(lifecycle[1].2, "think ");
    assert_eq!(lifecycle[2].2, "more");
    assert_eq!(lifecycle[3].0, "finish");
    assert!(lifecycle.iter().all(|event| event.1 == lifecycle[0].1));
}

#[test]
/// Verifies mixed reasoning and assistant output keeps independent IDs and finishes both streams.
fn mixed_reasoning_and_assistant_keep_independent_lifecycles() {
    let mut agent = Agent::new(FakeProvider::with_events(vec![
        ProviderStreamEvent::ReasoningDelta(ReasoningDelta {
            content: "think".to_owned(),
            metadata: None,
        }),
        ProviderStreamEvent::MessageDelta(MessageDelta::assistant("answer")),
        ProviderStreamEvent::Finished(finished_stop_with_usage()),
    ]));
    agent.enqueue_prompt("prompt");

    futures::executor::block_on(agent.run_next()).unwrap();

    let events = agent.events();
    let message_id = events.iter().find_map(|event| match event {
        AgentEvent::MessageStart { id } => Some(id.to_string()),
        _ => None,
    });
    let reasoning_id = events.iter().find_map(|event| match event {
        AgentEvent::ReasoningStart { id } => Some(id.to_string()),
        _ => None,
    });

    assert!(message_id.is_some());
    assert!(reasoning_id.is_some());
    assert_ne!(message_id, reasoning_id);
    assert!(events
        .iter()
        .any(|event| matches!(event, AgentEvent::MessageFinish { .. })));
    assert!(events
        .iter()
        .any(|event| matches!(event, AgentEvent::ReasoningFinish { .. })));
}

#[test]
/// Verifies tool execution emits explicit start and finish lifecycle events with preserved payloads.
fn tool_execution_emits_explicit_tool_lifecycle() {
    let provider = RecordingProvider::new(vec![
        vec![ProviderStreamEvent::Finished(ProviderFinished::tool_calls(
            vec![ProviderToolCall::new("call-1", "echo", r#"{"ok":true}"#)],
        ))],
        vec![
            ProviderStreamEvent::MessageDelta(MessageDelta::assistant("done")),
            ProviderStreamEvent::Finished(finished_stop_with_usage()),
        ],
    ]);
    let mut agent = Agent::new(provider);
    agent.register_tool(EchoTool).unwrap();
    agent.enqueue_prompt("prompt");

    futures::executor::block_on(agent.run_next()).unwrap();

    assert!(agent.events().windows(2).any(|events| matches!(
        events,
        [
            AgentEvent::ToolCallStart { tool_call_id, name, arguments },
            AgentEvent::ToolCallFinish { tool_call_id: finish_id, name: finish_name, output },
        ] if tool_call_id == "call-1"
            && finish_id == "call-1"
            && name == "echo"
            && finish_name == "echo"
            && arguments == r#"{"ok":true}"#
            && output == r#"{"ok":true}"#
    )));
}

#[test]
/// Verifies provider context reconstruction coalesces new message and tool lifecycle events.
fn context_reconstruction_uses_new_lifecycle_events() {
    let mut store = Store::default();
    store.append(AgentEvent::user_prompt("question"));
    store.append(AgentEvent::message_start("message-1"));
    store.append(AgentEvent::message_delta("message-1", "answer"));
    store.append(AgentEvent::message_finish("message-1"));
    store.append(AgentEvent::tool_call_start("call-1", "echo", "{}"));
    store.append(AgentEvent::tool_call_finish("call-1", "echo", "ok"));

    assert_eq!(
        provider_messages_from_store("system", &store),
        vec![
            ProviderMessage::system("system"),
            ProviderMessage::user("question"),
            ProviderMessage::assistant("answer"),
            ProviderMessage::assistant_tool_call(ProviderToolCall::new("call-1", "echo", "{}")),
            ProviderMessage::tool_result("call-1", "ok"),
        ]
    );
}

#[test]
/// Verifies provider terminal errors finish an active message before recording the error event.
fn provider_error_finishes_active_message_before_error_event() {
    let mut agent = Agent::new(FakeProvider::with_events(vec![
        ProviderStreamEvent::MessageDelta(MessageDelta::assistant("partial")),
        ProviderStreamEvent::Finished(ProviderFinished {
            finish_reason: FinishReason::Error,
            tool_calls: Vec::new(),
            usage: None,
            reasoning: None,
        }),
    ]));
    agent.enqueue_prompt("prompt");

    let _ = futures::executor::block_on(agent.run_next()).unwrap_err();
    assert_finish_precedes_terminal_error(&agent.events());
}

#[test]
/// Verifies provider stream errors close active lifecycle state before recording errors.
fn provider_stream_error_finishes_active_message_before_error_event() {
    let mut agent = Agent::new(StreamErrorProvider);
    agent.enqueue_prompt("prompt");

    let _ = futures::executor::block_on(agent.run_next()).unwrap_err();

    assert_finish_precedes_terminal_error(&agent.events());
}

/// Asserts a message finish boundary was recorded before the terminal error event.
fn assert_finish_precedes_terminal_error(events: &[AgentEvent]) {
    let finish_index = events
        .iter()
        .position(|event| matches!(event, AgentEvent::MessageFinish { .. }))
        .unwrap();
    let error_index = events
        .iter()
        .position(|event| matches!(event, AgentEvent::Error { .. }))
        .unwrap();

    assert!(finish_index < error_index);
}
