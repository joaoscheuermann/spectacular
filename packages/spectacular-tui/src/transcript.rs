use crate::ids::{Timestamp, TranscriptItemId};
use crate::render_model::RenderStyle;
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
    OpeningBanner(OpeningBannerItem),
    UserPrompt(UserPromptItem),
    AssistantMessage(AssistantMessageItem),
    Reasoning(ReasoningItem),
    ToolCall(ToolCallItem),
    Command(CommandItem),
    Error(ErrorItem),
    Warning(WarningItem),
    Success(SuccessItem),
    Notice(NoticeItem),
    Cancellation(CancellationItem),
    WorkedSummary(WorkedSummaryItem),
}

/// Opening session banner content with display-ready metadata.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OpeningBannerItem {
    pub version: String,
    pub model: String,
    pub reasoning: String,
    pub directory: String,
    pub session_id: String,
}

impl OpeningBannerItem {
    /// Creates an opening banner payload from display-ready session metadata.
    pub fn new(
        version: impl Into<String>,
        model: impl Into<String>,
        reasoning: impl Into<String>,
        directory: impl Into<String>,
        session_id: impl Into<String>,
    ) -> Self {
        Self {
            version: version.into(),
            model: model.into(),
            reasoning: reasoning.into(),
            directory: directory.into(),
            session_id: session_id.into(),
        }
    }
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<ToolDisplay>,
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
            display: None,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<CommandDisplay>,
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
            display: None,
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

/// Warning content recorded in the semantic transcript.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WarningItem {
    pub message: String,
}

impl WarningItem {
    /// Creates a warning transcript item payload.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Success content recorded in the semantic transcript.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SuccessItem {
    pub message: String,
}

impl SuccessItem {
    /// Creates a success transcript item payload.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
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

/// Cancellation content recorded in the semantic transcript.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CancellationItem {
    pub reason: String,
}

impl CancellationItem {
    /// Creates a cancellation transcript item payload.
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

/// Completed agent-work summary content recorded in the semantic transcript.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorkedSummaryItem {
    pub duration: String,
    pub turn_tokens: Option<u64>,
}

impl WorkedSummaryItem {
    /// Creates a completed work summary payload from display-ready duration and token count.
    pub fn new(duration: impl Into<String>, turn_tokens: Option<u64>) -> Self {
        Self {
            duration: duration.into(),
            turn_tokens,
        }
    }
}

/// One display-ready line with semantic style supplied by the runtime adapter.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DisplayLine {
    pub text: String,
    pub style: DisplayLineStyle,
}

impl DisplayLine {
    /// Creates a display-ready line from visible text and semantic style.
    pub fn new(text: impl Into<String>, style: DisplayLineStyle) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }
}

/// Serializable semantic style names for adapter-provided display payloads.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum DisplayLineStyle {
    Text,
    Dim,
    Tool,
    Command,
    CommandOutput,
    Success,
    Warning,
    Error,
    DiffAdded,
    DiffRemoved,
}

impl From<DisplayLineStyle> for RenderStyle {
    /// Maps persisted display styles into active render-model styles.
    fn from(style: DisplayLineStyle) -> Self {
        match style {
            DisplayLineStyle::Text => RenderStyle::Text,
            DisplayLineStyle::Dim => RenderStyle::Dim,
            DisplayLineStyle::Tool => RenderStyle::Tool,
            DisplayLineStyle::Command => RenderStyle::Command,
            DisplayLineStyle::CommandOutput => RenderStyle::CommandOutput,
            DisplayLineStyle::Success => RenderStyle::Success,
            DisplayLineStyle::Warning => RenderStyle::Warning,
            DisplayLineStyle::Error => RenderStyle::Error,
            DisplayLineStyle::DiffAdded => RenderStyle::DiffAdded,
            DisplayLineStyle::DiffRemoved => RenderStyle::DiffRemoved,
        }
    }
}

impl From<RenderStyle> for DisplayLineStyle {
    /// Maps active render-model styles into serializable display styles for tests and snapshots.
    fn from(style: RenderStyle) -> Self {
        match style {
            RenderStyle::Dim | RenderStyle::Reasoning => DisplayLineStyle::Dim,
            RenderStyle::Tool | RenderStyle::Task => DisplayLineStyle::Tool,
            RenderStyle::Command => DisplayLineStyle::Command,
            RenderStyle::CommandOutput => DisplayLineStyle::CommandOutput,
            RenderStyle::Success => DisplayLineStyle::Success,
            RenderStyle::Warning => DisplayLineStyle::Warning,
            RenderStyle::Error | RenderStyle::Secret => DisplayLineStyle::Error,
            RenderStyle::DiffAdded => DisplayLineStyle::DiffAdded,
            RenderStyle::DiffRemoved => DisplayLineStyle::DiffRemoved,
            _ => DisplayLineStyle::Text,
        }
    }
}

/// Display-ready tool payload persisted with a tool transcript item.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ToolDisplay {
    pub call_line: Option<DisplayLine>,
    #[serde(default)]
    pub argument_lines: Vec<DisplayLine>,
    #[serde(default)]
    pub output_lines: Vec<DisplayLine>,
}

/// Display-ready command payload persisted with a command transcript item.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandDisplay {
    pub command_line: Option<DisplayLine>,
    #[serde(default)]
    pub output_lines: Vec<DisplayLine>,
    pub summary_line: Option<DisplayLine>,
}

/// Display-focused status for tool call transcript items.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ToolStatus {
    Running,
    Finished,
    Failed,
}

/// Adapter-owned completion state for a display-ready tool result.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ToolDisplayStatus {
    Succeeded,
    Failed,
    Cancelled,
}

/// Adapter-owned completion state for a display-ready command result.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CommandDisplayStatus {
    Succeeded,
    Failed,
    Cancelled,
}

/// Display-focused status for command transcript items.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CommandStatus {
    Running,
    Finished,
    Failed,
}
