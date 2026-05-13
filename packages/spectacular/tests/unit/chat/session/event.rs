use super::*;
use serde_json::json;
use spectacular_agent::{provider_messages_from_store, CommandStatus, ContextSummary, Store};

/// Verifies that recognized JSONL event deserializes.
#[test]
fn recognized_jsonl_event_deserializes() {
    let event = ChatEvent::from_value(json!({
        "type": "user_prompt",
        "content": "hello",
        "created_at": "2026-04-29T14:01:00Z"
    }))
    .unwrap();

    assert_eq!(
        event,
        ChatEvent::UserPrompt {
            content: "hello".to_owned(),
            created_at: "2026-04-29T14:01:00Z".to_owned()
        }
    );
}

/// Verifies that unknown valid JSONL event is preserved.
#[test]
fn unknown_valid_jsonl_event_is_preserved() {
    let value = json!({"type": "future_event", "payload": true});

    assert_eq!(ChatEvent::from_value(value.clone()).unwrap_err(), value);
}

/// Verifies that minimal known events default optional fields.
#[test]
fn minimal_known_events_default_optional_fields() {
    let event = ChatEvent::from_value(json!({
        "type": "assistant_delta",
        "content": "hello",
        "created_at": "2026-04-29T14:01:00Z"
    }))
    .unwrap();

    assert_eq!(
        event,
        ChatEvent::AssistantDelta {
            role: "assistant".to_owned(),
            content: "hello".to_owned(),
            created_at: "2026-04-29T14:01:00Z".to_owned()
        }
    );
}

/// Verifies that agent event maps to existing wire shape.
#[test]
fn agent_event_maps_to_existing_wire_shape() {
    let event = ChatEvent::from_agent_event(
        &AgentEvent::MessageDelta(MessageDelta {
            role: ProviderMessageRole::Assistant,
            content: "hello".to_owned(),
        }),
        "2026-04-29T14:01:00Z".to_owned(),
    )
    .unwrap();
    let value = serde_json::to_value(event).unwrap();

    assert_eq!(
        value,
        json!({
            "type": "assistant_delta",
            "role": "assistant",
            "content": "hello",
            "created_at": "2026-04-29T14:01:00Z"
        })
    );
}

/// Verifies that reasoning delta round trips through JSONL agent event.
#[test]
fn reasoning_delta_round_trips_through_jsonl_agent_event() {
    let event = ChatEvent::from_agent_event(
        &AgentEvent::ReasoningDelta(ReasoningDelta {
            content: "thinking".to_owned(),
            metadata: None,
        }),
        "2026-04-29T14:01:00Z".to_owned(),
    )
    .unwrap();
    let value = serde_json::to_value(event).unwrap();

    assert_eq!(
        value,
        json!({
            "type": "reasoning_delta",
            "content": "thinking",
            "created_at": "2026-04-29T14:01:00Z"
        })
    );
    assert_eq!(
        ChatEvent::from_value(value)
            .unwrap()
            .to_agent_event()
            .unwrap(),
        AgentEvent::ReasoningDelta(ReasoningDelta {
            content: "thinking".to_owned(),
            metadata: None,
        })
    );
}

/// Verifies that content filter finish reason round trips.
#[test]
fn content_filter_finish_reason_round_trips() {
    let event = ChatEvent::from_agent_event(
        &AgentEvent::Finished {
            finish_reason: FinishReason::ContentFilter,
        },
        "2026-04-29T14:01:00Z".to_owned(),
    )
    .unwrap();

    assert!(matches!(
        event.to_agent_event(),
        Some(AgentEvent::Finished {
            finish_reason: FinishReason::ContentFilter
        })
    ));
}

/// Verifies that structured tool events round trip through JSONL to agent events.
#[test]
fn structured_tool_events_round_trip_through_jsonl_to_agent_events() {
    let events = vec![
        AgentEvent::assistant_tool_call_request("call-1", "write", r#"{"path":"foo.txt"}"#),
        AgentEvent::tool_result("call-1", "write", r#"{"success":true}"#),
    ];
    let lines = events
        .iter()
        .map(|event| {
            serde_json::to_string(
                &ChatEvent::from_agent_event(event, "2026-04-29T14:01:00Z".to_owned()).unwrap(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();

    let first_line: Value = serde_json::from_str(&lines[0]).unwrap();
    assert_eq!(
        first_line,
        json!({
            "type": "tool_call",
            "tool_call_id": "call-1",
            "name": "write",
            "arguments": r#"{"path":"foo.txt"}"#,
            "created_at": "2026-04-29T14:01:00Z"
        })
    );

    let round_trip = lines
        .iter()
        .map(|line| {
            let value = serde_json::from_str::<Value>(line).unwrap();
            ChatEvent::from_value(value)
                .unwrap()
                .to_agent_event()
                .unwrap()
        })
        .collect::<Vec<_>>();

    assert_eq!(round_trip, events);
}

/// Verifies command lifecycle events round trip through JSONL to agent events.
#[test]
fn command_lifecycle_events_round_trip_through_jsonl_to_agent_events() {
    let events = vec![
        AgentEvent::command_start(
            "cmd-1",
            "slash_command",
            "/git commit",
            "Git commit",
            "/git commit",
            Some("/repo".to_owned()),
        ),
        AgentEvent::command_delta("cmd-1", "status", "staged diff loaded", 1),
        AgentEvent::command_finished("cmd-1", CommandStatus::Success, "changes committed successfully"),
    ];

    let lines = events
        .iter()
        .map(|event| {
            serde_json::to_string(
                &ChatEvent::from_agent_event(event, "2026-04-29T14:01:00Z".to_owned()).unwrap(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();

    assert_eq!(
        serde_json::from_str::<Value>(&lines[0]).unwrap(),
        json!({
            "type": "command_start",
            "command_id": "cmd-1",
            "source": "slash_command",
            "name": "/git commit",
            "title": "Git commit",
            "command": "/git commit",
            "working_directory": "/repo",
            "created_at": "2026-04-29T14:01:00Z"
        })
    );
    assert_eq!(
        serde_json::from_str::<Value>(&lines[1]).unwrap(),
        json!({
            "type": "command_delta",
            "command_id": "cmd-1",
            "channel": "status",
            "content": "staged diff loaded",
            "sequence": 1,
            "created_at": "2026-04-29T14:01:00Z"
        })
    );
    assert_eq!(
        serde_json::from_str::<Value>(&lines[2]).unwrap(),
        json!({
            "type": "command_finished",
            "command_id": "cmd-1",
            "status": "success",
            "summary": "changes committed successfully",
            "created_at": "2026-04-29T14:01:00Z"
        })
    );

    let round_trip = lines
        .iter()
        .map(|line| {
            let value = serde_json::from_str::<Value>(line).unwrap();
            ChatEvent::from_value(value)
                .unwrap()
                .to_agent_event()
                .unwrap()
        })
        .collect::<Vec<_>>();

    assert_eq!(round_trip, events);
}

/// Verifies legacy command delta stream fields replay as command channels.
#[test]
fn legacy_command_delta_stream_replays_as_channel() {
    let event = ChatEvent::from_value(json!({
        "type": "command_delta",
        "command_id": "cmd-1",
        "stream": "status",
        "content": "staged diff loaded",
        "sequence": 1,
        "created_at": "2026-04-29T14:01:00Z"
    }))
    .unwrap();

    assert_eq!(
        event.to_agent_event(),
        Some(AgentEvent::command_delta(
            "cmd-1",
            "status",
            "staged diff loaded",
            1,
        ))
    );
}

/// Verifies command lifecycle events replay without provider-visible context.
#[test]
fn command_lifecycle_events_are_excluded_from_provider_messages() {
    let store = Store::from(vec![
        AgentEvent::user_prompt("write a commit"),
        AgentEvent::command_start(
            "cmd-1",
            "slash_command",
            "/git commit",
            "Git commit",
            "/git commit",
            None,
        ),
        AgentEvent::command_delta("cmd-1", "status", "generated commit message: fix: bug", 1),
        AgentEvent::command_finished("cmd-1", CommandStatus::Success, "changes committed successfully"),
        AgentEvent::MessageDelta(MessageDelta::assistant("done")),
    ]);

    let messages = provider_messages_from_store("system", &store);
    let text = messages
        .iter()
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(text.contains("system"));
    assert!(text.contains("write a commit"));
    assert!(text.contains("done"));
    assert!(!text.contains("generated commit message"));
    assert!(!text.contains("changes committed successfully"));
}

/// Verifies that legacy tool call content replays as structured agent event.
#[test]
fn legacy_tool_call_content_replays_as_structured_agent_event() {
    let event = ChatEvent::from_value(json!({
        "type": "tool_call",
        "content": r#"{"id":"call-1","name":"write","arguments":"{\"path\":\"foo.txt\"}"}"#,
        "created_at": "2026-04-29T14:01:00Z"
    }))
    .unwrap();

    assert_eq!(
        event.to_agent_event(),
        Some(AgentEvent::assistant_tool_call_request(
            "call-1",
            "write",
            r#"{"path":"foo.txt"}"#
        ))
    );
}

/// Verifies that structured tool events replay into provider messages.
#[test]
fn structured_tool_events_replay_into_provider_messages() {
    let records = [
        json!({
            "type": "tool_call",
            "tool_call_id": "call-1",
            "name": "write",
            "arguments": r#"{"path":"foo.txt"}"#,
            "created_at": "2026-04-29T14:01:00Z"
        }),
        json!({
            "type": "tool_result",
            "tool_call_id": "call-1",
            "name": "write",
            "content": r#"{"success":true}"#,
            "created_at": "2026-04-29T14:01:01Z"
        }),
    ];
    let events = records
        .into_iter()
        .map(|value| {
            ChatEvent::from_value(value)
                .unwrap()
                .to_agent_event()
                .unwrap()
        })
        .collect::<Vec<_>>();
    let store = Store::from(events);

    let messages = provider_messages_from_store("system", &store);

    assert_eq!(messages[1].tool_calls[0].id, "call-1");
    assert_eq!(messages[1].tool_calls[0].name, "write");
    assert_eq!(messages[1].tool_calls[0].arguments, r#"{"path":"foo.txt"}"#);
    assert_eq!(messages[2].tool_call_id.as_deref(), Some("call-1"));
    assert_eq!(messages[2].content, r#"{"success":true}"#);
}

/// Verifies that context summary round trips through JSONL to agent event.
#[test]
fn context_summary_round_trips_through_jsonl_to_agent_event() {
    let summary = AgentEvent::ContextSummaryCreated(ContextSummary {
        id: "summary-1".to_owned(),
        replaces: Some("summary-0".to_owned()),
        source_event_start: 0,
        source_event_end: 5,
        content: "# Goal\nKeep context compact.".to_owned(),
        estimated_tokens: 42,
    });
    let event =
        ChatEvent::from_agent_event(&summary, "2026-04-29T14:01:00Z".to_owned()).unwrap();
    let value = serde_json::to_value(event).unwrap();

    assert_eq!(
        value,
        json!({
            "type": "context_summary",
            "id": "summary-1",
            "replaces": "summary-0",
            "source_event_start": 0,
            "source_event_end": 5,
            "content": "# Goal\nKeep context compact.",
            "estimated_tokens": 42,
            "created_at": "2026-04-29T14:01:00Z"
        })
    );
    assert_eq!(
        ChatEvent::from_value(value)
            .unwrap()
            .to_agent_event()
            .unwrap(),
        summary
    );
}
