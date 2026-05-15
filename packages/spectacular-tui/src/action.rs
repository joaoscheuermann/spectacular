use crate::ids::{SessionId, TranscriptItemId};
use crate::metadata::{CommandDescriptor, ContextTokenUsage, DisplayMetadata, RuntimeSelection};
use crate::session::{PromptState, SelectionPromptState};

/// Events that can deterministically update TUI state through the reducer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChatTuiAction {
    PromptChanged(PromptState),
    SubmitPrompt {
        id: TranscriptItemId,
        text: String,
    },
    CancelRun,
    SelectionPromptChanged(Option<SelectionPromptState>),
    SelectionPromptSubmitted(SelectionPromptAnswer),
    SelectionPromptCancelled,
    CommandsLoaded(Vec<CommandDescriptor>),
    SessionChanged {
        id: SessionId,
    },
    AgentStarted,
    MessageStarted {
        id: TranscriptItemId,
    },
    MessageDelta {
        id: TranscriptItemId,
        text: String,
    },
    MessageFinished {
        id: TranscriptItemId,
    },
    ReasoningStarted {
        id: TranscriptItemId,
    },
    ReasoningDelta {
        id: TranscriptItemId,
        text: String,
    },
    ReasoningFinished {
        id: TranscriptItemId,
    },
    ToolCallStarted {
        id: TranscriptItemId,
        tool_call_id: String,
        name: String,
        arguments: String,
    },
    ToolCallDelta {
        tool_call_id: String,
        text: String,
    },
    ToolCallFinished {
        tool_call_id: String,
        name: String,
        output: String,
    },
    CommandStarted {
        id: TranscriptItemId,
        command_id: String,
        command: String,
    },
    CommandOutput {
        command_id: String,
        text: String,
    },
    CommandFinished {
        command_id: String,
        exit_code: Option<i32>,
    },
    AgentFinished,
    WorkedSummaryReported {
        duration: String,
        turn_tokens: Option<u64>,
    },
    AgentFailed {
        message: String,
    },
    AgentCancelled {
        reason: String,
    },
    ErrorReported {
        message: String,
        details: Option<String>,
    },
    NoticeReported {
        message: String,
    },
    RuntimeSelectionChanged(RuntimeSelection),
    DisplayMetadataChanged(DisplayMetadata),
    UsageUpdated(ContextTokenUsage),
    SpinnerTick,
    ScrollTranscript(i32),
    Resize {
        width: u16,
        height: u16,
    },
}

/// Answer returned from an interactive selection prompt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectionPromptAnswer {
    pub choice: SelectionPromptChoice,
    pub comment: Option<String>,
}

/// Selected predefined option or custom free-text selection value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SelectionPromptChoice {
    Option { index: usize, label: String },
    Custom(String),
}
