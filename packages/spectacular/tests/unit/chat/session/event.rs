use super::*;
use serde_json::json;
use spectacular_agent::{provider_messages_from_store, Store};

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

#[test]
fn unknown_valid_jsonl_event_is_preserved() {
    let value = json!({"type": "future_event", "payload": true});

    assert_eq!(ChatEvent::from_value(value.clone()).unwrap_err(), value);
}

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
