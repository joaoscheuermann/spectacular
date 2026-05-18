//! Chat session JSONL event schema.
//!
//! New sessions use schema version 2 so tool calls and tool results are stored
//! as structured records: `tool_call_id`, `name`, `arguments`, and provider
//! visible `content`. Older `tool_call.content` records are normalized on read
//! so old JSONL sessions can still replay into structured agent events.

use crate::chat::command_event::{
    CommandDelta, CommandEvent, CommandFinished, CommandStart, CommandStatus,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use spectacular_agent::{AgentEvent, AgentTranscriptItemId, ContextSummary};
use spectacular_llms::{FinishReason, ProviderMessageRole};

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
    UserPrompt {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        content: String,
        created_at: String,
    },
    #[serde(rename = "message_start")]
    MessageStart { id: String, created_at: String },
    #[serde(rename = "assistant_delta")]
    AssistantDelta {
        #[serde(default = "assistant_role")]
        role: String,
        #[serde(default = "session_replay_message_id")]
        id: String,
        content: String,
        created_at: String,
    },
    #[serde(rename = "message_finish")]
    MessageFinish { id: String, created_at: String },
    #[serde(rename = "reasoning_start")]
    ReasoningStart { id: String, created_at: String },
    #[serde(rename = "reasoning_delta")]
    ReasoningDelta {
        #[serde(default = "session_replay_reasoning_id")]
        id: String,
        content: String,
        created_at: String,
    },
    #[serde(rename = "reasoning_finish")]
    ReasoningFinish { id: String, created_at: String },
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
    #[serde(rename = "command_start")]
    CommandStart {
        command_id: String,
        source: String,
        name: String,
        title: String,
        command: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        working_directory: Option<String>,
        created_at: String,
    },
    #[serde(rename = "command_delta")]
    CommandDelta {
        command_id: String,
        #[serde(alias = "stream")]
        channel: String,
        content: String,
        sequence: u64,
        created_at: String,
    },
    #[serde(rename = "command_finished")]
    CommandFinished {
        command_id: String,
        status: String,
        summary: String,
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
    #[serde(rename = "context_summary")]
    ContextSummary {
        id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        replaces: Option<String>,
        source_event_start: usize,
        source_event_end: usize,
        content: String,
        estimated_tokens: usize,
        created_at: String,
    },
}

impl ChatEvent {
    /// Deserializes a chat event value while preserving unknown input on failure.
    pub fn from_value(value: Value) -> Result<Self, Value> {
        let original = value.clone();
        let value = normalize_legacy_tool_call(value);
        serde_json::from_value(value).map_err(|_| original)
    }

    /// Converts an agent event into a persisted chat event when it is session-visible.
    pub fn from_agent_event(event: &AgentEvent, created_at: String) -> Option<Self> {
        match event {
            AgentEvent::UserPrompt { id, content } => Some(Self::UserPrompt {
                id: id.as_ref().map(|id| id.as_str().to_owned()),
                content: content.clone(),
                created_at,
            }),
            AgentEvent::MessageStart { id } => Some(Self::MessageStart {
                id: id.as_str().to_owned(),
                created_at,
            }),
            AgentEvent::MessageDelta { id, content } => Some(Self::AssistantDelta {
                role: role(ProviderMessageRole::Assistant).to_owned(),
                id: id.as_str().to_owned(),
                content: content.clone(),
                created_at,
            }),
            AgentEvent::MessageFinish { id } => Some(Self::MessageFinish {
                id: id.as_str().to_owned(),
                created_at,
            }),
            AgentEvent::ReasoningStart { id } => Some(Self::ReasoningStart {
                id: id.as_str().to_owned(),
                created_at,
            }),
            AgentEvent::ReasoningDelta { id, content } => Some(Self::ReasoningDelta {
                id: id.as_str().to_owned(),
                content: content.clone(),
                created_at,
            }),
            AgentEvent::ReasoningFinish { id } => Some(Self::ReasoningFinish {
                id: id.as_str().to_owned(),
                created_at,
            }),
            AgentEvent::ToolCallStart {
                tool_call_id,
                name,
                arguments,
            } => Some(Self::ToolCall {
                tool_call_id: tool_call_id.clone(),
                name: name.clone(),
                arguments: arguments.clone(),
                created_at,
            }),
            AgentEvent::ToolCallFinish {
                tool_call_id,
                name,
                output,
            } => Some(Self::ToolResult {
                tool_call_id: tool_call_id.clone(),
                name: name.clone(),
                content: output.clone(),
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
            AgentEvent::ContextSummaryCreated(summary) => Some(Self::ContextSummary {
                id: summary.id.clone(),
                replaces: summary.replaces.clone(),
                source_event_start: summary.source_event_start,
                source_event_end: summary.source_event_end,
                content: summary.content.clone(),
                estimated_tokens: summary.estimated_tokens,
                created_at,
            }),
            AgentEvent::ReasoningMetadata(_)
            | AgentEvent::ContextTokenUsage(_)
            | AgentEvent::Internal { .. } => None,
            _ => None,
        }
    }

    /// Converts an app-owned command lifecycle event into a persisted chat event.
    pub fn from_command_event(event: &CommandEvent, created_at: String) -> Self {
        match event {
            CommandEvent::Start(start) => Self::CommandStart {
                command_id: start.command_id.clone(),
                source: start.source.clone(),
                name: start.name.clone(),
                title: start.title.clone(),
                command: start.command.clone(),
                working_directory: start.working_directory.clone(),
                created_at,
            },
            CommandEvent::Delta(delta) => Self::CommandDelta {
                command_id: delta.command_id.clone(),
                channel: delta.channel.clone(),
                content: delta.content.clone(),
                sequence: delta.sequence,
                created_at,
            },
            CommandEvent::Finished(finished) => Self::CommandFinished {
                command_id: finished.command_id.clone(),
                status: finished.status.as_str().to_owned(),
                summary: finished.summary.clone(),
                created_at,
            },
        }
    }

    /// Converts a persisted chat event back into an app-owned command lifecycle event.
    pub fn to_command_event(&self) -> Option<CommandEvent> {
        match self {
            Self::CommandStart {
                command_id,
                source,
                name,
                title,
                command,
                working_directory,
                ..
            } => Some(CommandEvent::Start(CommandStart {
                command_id: command_id.clone(),
                source: source.clone(),
                name: name.clone(),
                title: title.clone(),
                command: command.clone(),
                working_directory: working_directory.clone(),
            })),
            Self::CommandDelta {
                command_id,
                channel,
                content,
                sequence,
                ..
            } => Some(CommandEvent::Delta(CommandDelta {
                command_id: command_id.clone(),
                channel: channel.clone(),
                content: content.clone(),
                sequence: *sequence,
            })),
            Self::CommandFinished {
                command_id,
                status,
                summary,
                ..
            } => Some(CommandEvent::Finished(CommandFinished {
                command_id: command_id.clone(),
                status: CommandStatus::from_str(status),
                summary: summary.clone(),
            })),
            _ => None,
        }
    }

    /// Converts a persisted chat event back into an agent event when replayable.
    pub fn to_agent_event(&self) -> Option<AgentEvent> {
        match self {
            Self::UserPrompt { id, content, .. } => Some(AgentEvent::UserPrompt {
                id: id.as_ref().map(AgentTranscriptItemId::new),
                content: content.clone(),
            }),
            Self::MessageStart { id, .. } => Some(AgentEvent::message_start(id.clone())),
            Self::AssistantDelta { id, content, .. } => {
                Some(AgentEvent::message_delta(id.clone(), content.clone()))
            }
            Self::MessageFinish { id, .. } => Some(AgentEvent::message_finish(id.clone())),
            Self::ReasoningStart { id, .. } => Some(AgentEvent::reasoning_start(id.clone())),
            Self::ReasoningDelta { id, content, .. } => {
                Some(AgentEvent::reasoning_delta(id.clone(), content.clone()))
            }
            Self::ReasoningFinish { id, .. } => Some(AgentEvent::reasoning_finish(id.clone())),
            Self::ToolCall {
                tool_call_id,
                name,
                arguments,
                ..
            } => Some(AgentEvent::tool_call_start(
                tool_call_id.clone(),
                name.clone(),
                arguments.clone(),
            )),
            Self::ToolResult {
                tool_call_id,
                name,
                content,
                ..
            } => Some(AgentEvent::tool_call_finish(
                tool_call_id.clone(),
                name.clone(),
                content.clone(),
            )),
            Self::CommandStart { .. }
            | Self::CommandDelta { .. }
            | Self::CommandFinished { .. } => None,
            Self::ValidationError { message, .. } => Some(AgentEvent::validation_error(message)),
            Self::Error { message, .. } => Some(AgentEvent::error(message)),
            Self::Cancelled { reason, .. } => Some(AgentEvent::cancelled(reason)),
            Self::Finished { reason, .. } => Some(AgentEvent::Finished {
                finish_reason: finish_reason_from_str(reason),
            }),
            Self::ContextSummary {
                id,
                replaces,
                source_event_start,
                source_event_end,
                content,
                estimated_tokens,
                ..
            } => Some(AgentEvent::ContextSummaryCreated(ContextSummary {
                id: id.clone(),
                replaces: replaces.clone(),
                source_event_start: *source_event_start,
                source_event_end: *source_event_end,
                content: content.clone(),
                estimated_tokens: *estimated_tokens,
            })),
            Self::SessionStarted { .. }
            | Self::ProviderChanged { .. }
            | Self::ModelChanged { .. }
            | Self::SessionTitleUpdated { .. }
            | Self::UsageMetadata { .. } => None,
        }
    }

    /// Parses the event creation timestamp as UTC when present.
    pub fn created_at(&self) -> Option<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(self.created_at_str()?)
            .ok()
            .map(|value| value.with_timezone(&Utc))
    }

    /// Returns the raw event creation timestamp when present.
    pub fn created_at_str(&self) -> Option<&str> {
        match self {
            Self::SessionStarted { created_at, .. }
            | Self::ProviderChanged { created_at, .. }
            | Self::ModelChanged { created_at, .. }
            | Self::SessionTitleUpdated { created_at, .. }
            | Self::UserPrompt { created_at, .. }
            | Self::MessageStart { created_at, .. }
            | Self::AssistantDelta { created_at, .. }
            | Self::MessageFinish { created_at, .. }
            | Self::ReasoningStart { created_at, .. }
            | Self::ReasoningDelta { created_at, .. }
            | Self::ReasoningFinish { created_at, .. }
            | Self::ToolCall { created_at, .. }
            | Self::ToolResult { created_at, .. }
            | Self::CommandStart { created_at, .. }
            | Self::CommandDelta { created_at, .. }
            | Self::CommandFinished { created_at, .. }
            | Self::UsageMetadata { created_at, .. }
            | Self::ValidationError { created_at, .. }
            | Self::Error { created_at, .. }
            | Self::Cancelled { created_at, .. }
            | Self::Finished { created_at, .. }
            | Self::ContextSummary { created_at, .. } => Some(created_at),
        }
    }

    /// Returns whether this event records a user prompt.
    pub fn is_user_prompt(&self) -> bool {
        matches!(self, Self::UserPrompt { .. })
    }

    /// Returns the prompt content when this event is a user prompt.
    pub fn user_prompt(&self) -> Option<&str> {
        match self {
            Self::UserPrompt { content, .. } => Some(content),
            _ => None,
        }
    }
}

/// Converts a provider message role into its serialized chat role.
fn role(role: ProviderMessageRole) -> &'static str {
    match role {
        ProviderMessageRole::System => "system",
        ProviderMessageRole::User => "user",
        ProviderMessageRole::Assistant => "assistant",
        ProviderMessageRole::Tool => "tool",
    }
}

/// Converts a finish reason into its persisted string form.
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

/// Converts a persisted finish reason string into a provider finish reason.
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

/// Returns the default title used for sessions without a stored title.
fn untitled() -> String {
    "Untitled session".to_owned()
}

/// Returns the default assistant role used for legacy assistant deltas.
fn assistant_role() -> String {
    "assistant".to_owned()
}

/// Returns the replay ID used for legacy assistant delta records.
fn session_replay_message_id() -> String {
    "session-replay-message".to_owned()
}

/// Returns the replay ID used for legacy reasoning delta records.
fn session_replay_reasoning_id() -> String {
    "session-replay-reasoning".to_owned()
}

/// Normalizes legacy tool-call JSON into the structured session schema.
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
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/session/event.rs"
    ));
}
