use crate::ids::{Timestamp, TranscriptItemId};
use serde::{Deserialize, Serialize};

/// One semantic renderable unit in the conversation transcript.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TranscriptItem {
    pub id: TranscriptItemId,
    pub timestamp: Timestamp,
    pub content: TranscriptItemContent,
}

impl TranscriptItem {
    /// Creates a transcript item from identity, timestamp, and semantic content.
    pub fn new(id: TranscriptItemId, timestamp: Timestamp, content: TranscriptItemContent) -> Self {
        Self {
            id,
            timestamp,
            content,
        }
    }
}

/// Semantic transcript content that renderers can project into UI blocks.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "data")]
pub enum TranscriptItemContent {
    UserPrompt(UserPromptItem),
    AssistantMessage(AssistantMessageItem),
    Reasoning(ReasoningItem),
    ToolCall(ToolCallItem),
    Command(CommandItem),
    Error(ErrorItem),
    Notice(NoticeItem),
}

/// User-authored prompt content submitted into a session.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct UserPromptItem {
    pub text: String,
}

impl UserPromptItem {
    /// Creates a user prompt transcript item payload.
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

/// Assistant-authored message content accumulated from lifecycle deltas.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AssistantMessageItem {
    pub text: String,
}

impl AssistantMessageItem {
    /// Creates an assistant message transcript item payload.
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

/// Reasoning content accumulated from lifecycle deltas.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReasoningItem {
    pub text: String,
    pub collapsed: bool,
}

impl ReasoningItem {
    /// Creates a reasoning transcript item payload with collapsed display state.
    pub fn new(text: impl Into<String>, collapsed: bool) -> Self {
        Self {
            text: text.into(),
            collapsed,
        }
    }
}

/// Tool-call transcript content with display-focused lifecycle status.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ToolCallItem {
    pub tool_call_id: String,
    pub name: String,
    pub arguments_preview: Option<String>,
    pub status: ToolStatus,
    pub output_preview: Option<String>,
}

impl ToolCallItem {
    /// Creates a running tool-call transcript item payload.
    pub fn running(
        tool_call_id: impl Into<String>,
        name: impl Into<String>,
        arguments_preview: Option<String>,
    ) -> Self {
        Self {
            tool_call_id: tool_call_id.into(),
            name: name.into(),
            arguments_preview,
            status: ToolStatus::Running,
            output_preview: None,
        }
    }
}

/// Command transcript content with accumulated output and exit status.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandItem {
    pub command_id: String,
    pub command: String,
    pub status: CommandStatus,
    pub output: String,
    pub exit_code: Option<i32>,
}

impl CommandItem {
    /// Creates a running command transcript item payload.
    pub fn running(command_id: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            command_id: command_id.into(),
            command: command.into(),
            status: CommandStatus::Running,
            output: String::new(),
            exit_code: None,
        }
    }
}

/// Error content recorded in the semantic transcript.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ErrorItem {
    pub message: String,
    pub details: Option<String>,
}

impl ErrorItem {
    /// Creates an error transcript item payload.
    pub fn new(message: impl Into<String>, details: Option<String>) -> Self {
        Self {
            message: message.into(),
            details,
        }
    }
}

/// Notice content recorded in the semantic transcript.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NoticeItem {
    pub message: String,
}

impl NoticeItem {
    /// Creates a notice transcript item payload.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Display-focused status for tool call transcript items.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ToolStatus {
    Running,
    Finished,
    Failed,
}

/// Display-focused status for command transcript items.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CommandStatus {
    Running,
    Finished,
    Failed,
}
