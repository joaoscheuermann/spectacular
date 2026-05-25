//! Chat session JSONL event schema.
//!
//! New sessions use schema version 2 so tool calls and tool results are stored
//! as structured records: `tool_call_id`, `name`, `arguments`, and provider
//! visible `content`. Older `tool_call.content` records are normalized on read
//! so old JSONL sessions can still replay into structured agent events.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use spectacular_agent::AgentErrorDetails;

mod agent;
mod command;
mod wire;

use wire::{assistant_role, session_replay_message_id, session_replay_reasoning_id, untitled};

/// Persisted chat-session event schema used for JSONL transcripts.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum ChatEvent {
    /// Marks the start of a session and declares the event schema version.
    #[serde(rename = "session_started")]
    SessionStarted {
        schema_version: u64,
        id: String,
        #[serde(default = "untitled")]
        title: String,
        created_at: String,
    },
    /// Records the active provider selected for subsequent chat turns.
    #[serde(rename = "provider_changed")]
    ProviderChanged {
        provider: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        source: Option<String>,
        created_at: String,
    },
    /// Records the model and reasoning settings selected for a task slot.
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
    /// Updates the display title associated with the session.
    #[serde(rename = "session_title_updated")]
    SessionTitleUpdated {
        title: String,
        slot: String,
        model: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        source: Option<String>,
        created_at: String,
    },
    /// Stores a prompt submitted by the user.
    #[serde(rename = "user_prompt")]
    UserPrompt {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        content: String,
        created_at: String,
    },
    /// Marks the start of an assistant message stream.
    #[serde(rename = "message_start")]
    MessageStart { id: String, created_at: String },
    /// Stores an assistant message content delta.
    #[serde(rename = "assistant_delta")]
    AssistantDelta {
        #[serde(default = "assistant_role")]
        role: String,
        #[serde(default = "session_replay_message_id")]
        id: String,
        content: String,
        created_at: String,
    },
    /// Marks the end of an assistant message stream.
    #[serde(rename = "message_finish")]
    MessageFinish { id: String, created_at: String },
    /// Marks the start of a reasoning stream.
    #[serde(rename = "reasoning_start")]
    ReasoningStart { id: String, created_at: String },
    /// Stores a reasoning content delta.
    #[serde(rename = "reasoning_delta")]
    ReasoningDelta {
        #[serde(default = "session_replay_reasoning_id")]
        id: String,
        content: String,
        created_at: String,
    },
    /// Marks the end of a reasoning stream.
    #[serde(rename = "reasoning_finish")]
    ReasoningFinish { id: String, created_at: String },
    /// Stores the structured arguments for a provider tool call.
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
    /// Stores the output returned by a tool call.
    #[serde(rename = "tool_result")]
    ToolResult {
        #[serde(default)]
        tool_call_id: String,
        #[serde(default)]
        name: String,
        content: String,
        created_at: String,
    },
    /// Marks the start of an app-owned command lifecycle.
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
    /// Stores a command output delta.
    #[serde(rename = "command_delta")]
    CommandDelta {
        command_id: String,
        #[serde(alias = "stream")]
        channel: String,
        content: String,
        sequence: u64,
        created_at: String,
    },
    /// Marks completion of an app-owned command lifecycle.
    #[serde(rename = "command_finished")]
    CommandFinished {
        command_id: String,
        status: String,
        summary: String,
        created_at: String,
    },
    /// Stores token usage metadata reported by the provider.
    #[serde(rename = "usage_metadata")]
    UsageMetadata {
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        total_tokens: Option<u64>,
        created_at: String,
    },
    /// Stores a validation failure emitted before provider execution.
    #[serde(rename = "validation_error")]
    ValidationError { message: String, created_at: String },
    /// Stores an agent or provider error.
    #[serde(rename = "error")]
    Error {
        message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        details: Option<AgentErrorDetails>,
        created_at: String,
    },
    /// Records cancellation of the active turn.
    #[serde(rename = "cancelled")]
    Cancelled { reason: String, created_at: String },
    /// Records normal completion of the active turn.
    #[serde(rename = "finished")]
    Finished { reason: String, created_at: String },
    /// Stores a context summary that can replace an older event range.
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

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/session/event.rs"
    ));
}
