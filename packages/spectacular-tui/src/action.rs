use crate::metadata::{CommandDescriptor, ContextTokenUsage, DisplayMetadata, RuntimeSelection};
use crate::session::{PromptState, SessionId};

/// Events that can deterministically update TUI state through the reducer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChatTuiAction {
    PromptChanged(PromptState),
    SubmitPrompt(String),
    CancelRun,
    CommandsLoaded(Vec<CommandDescriptor>),
    SessionChanged { id: SessionId },
    AgentStarted,
    AgentFinished,
    AgentFailed { message: String },
    AgentCancelled { reason: String },
    RuntimeSelectionChanged(RuntimeSelection),
    DisplayMetadataChanged(DisplayMetadata),
    UsageUpdated(ContextTokenUsage),
    SpinnerTick,
    ScrollTranscript(i32),
    Resize,
}
