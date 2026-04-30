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
    ToolCall { content: String, created_at: String },
    #[serde(rename = "tool_result")]
    ToolResult { content: String, created_at: String },
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
        serde_json::from_value(value.clone()).map_err(|_| value)
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
            AgentEvent::AssistantToolCallRequest { content } => Some(Self::ToolCall {
                content: content.clone(),
                created_at,
            }),
            AgentEvent::ToolResult { content } => Some(Self::ToolResult {
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
            Self::ToolCall { content, .. } => {
                Some(AgentEvent::assistant_tool_call_request(content))
            }
            Self::ToolResult { content, .. } => Some(AgentEvent::tool_result(content)),
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
        FinishReason::Error => "error",
    }
}

fn finish_reason_from_str(reason: &str) -> FinishReason {
    match reason {
        "length" => FinishReason::Length,
        "toolcalls" | "tool_calls" => FinishReason::ToolCalls,
        "cancelled" => FinishReason::Cancelled,
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
}
