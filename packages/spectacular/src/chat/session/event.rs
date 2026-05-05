//! Chat session JSONL event schema.
//!
//! New sessions use schema version 2 so tool calls and tool results are stored
//! as structured records: `tool_call_id`, `name`, `arguments`, and provider
//! visible `content`. Older `tool_call.content` records are normalized on read
//! so old JSONL sessions can still replay into structured agent events.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use spectacular_agent::AgentEvent;
use spectacular_llms::{FinishReason, MessageDelta, ProviderMessageRole, ReasoningDelta};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum ChatEvent {
    #[serde(rename = "session_started")]
    SessionStarted {
        schema_version: u64,
        id: String,
        #[serde(default = "untitled")]
        title: String,
        created_at: String,
    },
    #[serde(rename = "provider_changed")]
    ProviderChanged {
        provider: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        source: Option<String>,
        created_at: String,
    },
    #[serde(rename = "model_changed")]
    ModelChanged {
        slot: String,
        provider: String,
        model: String,
        reasoning: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        source: Option<String>,
        created_at: String,
    },
    #[serde(rename = "session_title_updated")]
    SessionTitleUpdated {
        title: String,
        slot: String,
        model: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        source: Option<String>,
        created_at: String,
    },
    #[serde(rename = "user_prompt")]
    UserPrompt { content: String, created_at: String },
    #[serde(rename = "assistant_delta")]
    AssistantDelta {
        #[serde(default = "assistant_role")]
        role: String,
        content: String,
        created_at: String,
    },
    #[serde(rename = "reasoning_delta")]
    ReasoningDelta { content: String, created_at: String },
    #[serde(rename = "tool_call")]
    ToolCall {
        #[serde(default)]
        tool_call_id: String,
        #[serde(default)]
        name: String,
        #[serde(default)]
        arguments: String,
        created_at: String,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        #[serde(default)]
        tool_call_id: String,
        #[serde(default)]
        name: String,
        content: String,
        created_at: String,
    },
    #[serde(rename = "usage_metadata")]
    UsageMetadata {
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        total_tokens: Option<u64>,
        created_at: String,
    },
    #[serde(rename = "validation_error")]
    ValidationError { message: String, created_at: String },
    #[serde(rename = "error")]
    Error { message: String, created_at: String },
    #[serde(rename = "cancelled")]
    Cancelled { reason: String, created_at: String },
    #[serde(rename = "finished")]
    Finished { reason: String, created_at: String },
}

impl ChatEvent {
    pub fn from_value(value: Value) -> Result<Self, Value> {
        let original = value.clone();
        let value = normalize_legacy_tool_call(value);
        serde_json::from_value(value).map_err(|_| original)
    }

    pub fn from_agent_event(event: &AgentEvent, created_at: String) -> Option<Self> {
        match event {
            AgentEvent::UserPrompt { content } => Some(Self::UserPrompt {
                content: content.clone(),
                created_at,
            }),
            AgentEvent::MessageDelta(delta) => Some(Self::AssistantDelta {
                role: role(delta.role).to_owned(),
                content: delta.content.clone(),
                created_at,
            }),
            AgentEvent::ReasoningDelta(delta) => Some(Self::ReasoningDelta {
                content: delta.content.clone(),
                created_at,
            }),
            AgentEvent::AssistantToolCallRequest {
                tool_call_id,
                name,
                arguments,
            } => Some(Self::ToolCall {
                tool_call_id: tool_call_id.clone(),
                name: name.clone(),
                arguments: arguments.clone(),
                created_at,
            }),
            AgentEvent::ToolResult {
                tool_call_id,
                name,
                content,
            } => Some(Self::ToolResult {
                tool_call_id: tool_call_id.clone(),
                name: name.clone(),
                content: content.clone(),
                created_at,
            }),
            AgentEvent::UsageMetadata(usage) => Some(Self::UsageMetadata {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                total_tokens: usage.total_tokens,
                created_at,
            }),
            AgentEvent::ValidationError { message } => Some(Self::ValidationError {
                message: message.clone(),
                created_at,
            }),
            AgentEvent::Error { message } => Some(Self::Error {
                message: message.clone(),
                created_at,
            }),
            AgentEvent::Cancelled { reason } => Some(Self::Cancelled {
                reason: reason.clone(),
                created_at,
            }),
            AgentEvent::Finished { finish_reason } => Some(Self::Finished {
                reason: finish_reason_to_str(*finish_reason).to_owned(),
                created_at,
            }),
            AgentEvent::ReasoningMetadata(_) | AgentEvent::Internal { .. } => None,
        }
    }

    pub fn to_agent_event(&self) -> Option<AgentEvent> {
        match self {
            Self::UserPrompt { content, .. } => Some(AgentEvent::user_prompt(content)),
            Self::AssistantDelta { role, content, .. } => {
                Some(AgentEvent::MessageDelta(MessageDelta {
                    role: provider_role(role),
                    content: content.clone(),
                }))
            }
            Self::ReasoningDelta { content, .. } => {
                Some(AgentEvent::ReasoningDelta(ReasoningDelta {
                    content: content.clone(),
                    metadata: None,
                }))
            }
            Self::ToolCall {
                tool_call_id,
                name,
                arguments,
                ..
            } => Some(AgentEvent::assistant_tool_call_request(
                tool_call_id.clone(),
                name.clone(),
                arguments.clone(),
            )),
            Self::ToolResult {
                tool_call_id,
                name,
                content,
                ..
            } => Some(AgentEvent::tool_result(
                tool_call_id.clone(),
                name.clone(),
                content.clone(),
            )),
            Self::ValidationError { message, .. } => Some(AgentEvent::validation_error(message)),
            Self::Error { message, .. } => Some(AgentEvent::error(message)),
            Self::Cancelled { reason, .. } => Some(AgentEvent::cancelled(reason)),
            Self::Finished { reason, .. } => Some(AgentEvent::Finished {
                finish_reason: finish_reason_from_str(reason),
            }),
            Self::SessionStarted { .. }
            | Self::ProviderChanged { .. }
            | Self::ModelChanged { .. }
            | Self::SessionTitleUpdated { .. }
            | Self::UsageMetadata { .. } => None,
        }
    }

    pub fn created_at(&self) -> Option<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(self.created_at_str()?)
            .ok()
            .map(|value| value.with_timezone(&Utc))
    }

    pub fn created_at_str(&self) -> Option<&str> {
        match self {
            Self::SessionStarted { created_at, .. }
            | Self::ProviderChanged { created_at, .. }
            | Self::ModelChanged { created_at, .. }
            | Self::SessionTitleUpdated { created_at, .. }
            | Self::UserPrompt { created_at, .. }
            | Self::AssistantDelta { created_at, .. }
            | Self::ReasoningDelta { created_at, .. }
            | Self::ToolCall { created_at, .. }
            | Self::ToolResult { created_at, .. }
            | Self::UsageMetadata { created_at, .. }
            | Self::ValidationError { created_at, .. }
            | Self::Error { created_at, .. }
            | Self::Cancelled { created_at, .. }
            | Self::Finished { created_at, .. } => Some(created_at),
        }
    }

    pub fn is_user_prompt(&self) -> bool {
        matches!(self, Self::UserPrompt { .. })
    }

    pub fn user_prompt(&self) -> Option<&str> {
        match self {
            Self::UserPrompt { content, .. } => Some(content),
            _ => None,
        }
    }
}

fn role(role: ProviderMessageRole) -> &'static str {
    match role {
        ProviderMessageRole::System => "system",
        ProviderMessageRole::User => "user",
        ProviderMessageRole::Assistant => "assistant",
        ProviderMessageRole::Tool => "tool",
    }
}

fn provider_role(role: &str) -> ProviderMessageRole {
    match role {
        "system" => ProviderMessageRole::System,
        "user" => ProviderMessageRole::User,
        "tool" => ProviderMessageRole::Tool,
        _ => ProviderMessageRole::Assistant,
    }
}

fn finish_reason_to_str(reason: FinishReason) -> &'static str {
    match reason {
        FinishReason::Stop => "stop",
        FinishReason::Length => "length",
        FinishReason::ToolCalls => "toolcalls",
        FinishReason::Cancelled => "cancelled",
        FinishReason::ContentFilter => "content_filter",
        FinishReason::Error => "error",
    }
}

fn finish_reason_from_str(reason: &str) -> FinishReason {
    match reason {
        "length" => FinishReason::Length,
        "toolcalls" | "tool_calls" => FinishReason::ToolCalls,
        "cancelled" => FinishReason::Cancelled,
        "content_filter" => FinishReason::ContentFilter,
        "error" => FinishReason::Error,
        _ => FinishReason::Stop,
    }
}

fn untitled() -> String {
    "Untitled session".to_owned()
}

fn assistant_role() -> String {
    "assistant".to_owned()
}

fn normalize_legacy_tool_call(mut value: Value) -> Value {
    let Some(object) = value.as_object_mut() else {
        return value;
    };
    if object.get("type").and_then(Value::as_str) != Some("tool_call") {
        return value;
    }
    if object
        .get("tool_call_id")
        .or_else(|| object.get("name"))
        .or_else(|| object.get("arguments"))
        .is_some()
    {
        return value;
    }

    let Some(content) = object.get("content").and_then(Value::as_str) else {
        return value;
    };
    let Ok(content) = serde_json::from_str::<Value>(content) else {
        return value;
    };

    if let Some(tool_call_id) = content.get("id").and_then(Value::as_str) {
        object.insert(
            "tool_call_id".to_owned(),
            Value::String(tool_call_id.to_owned()),
        );
    }
    if let Some(name) = content.get("name").and_then(Value::as_str) {
        object.insert("name".to_owned(), Value::String(name.to_owned()));
    }
    if let Some(arguments) = content.get("arguments").and_then(Value::as_str) {
        object.insert("arguments".to_owned(), Value::String(arguments.to_owned()));
    }

    value
}

#[cfg(test)]
mod tests {
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
}
