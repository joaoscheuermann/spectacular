pub(crate) mod display;
pub(crate) mod lookup;

mod prompt;
mod status;
mod transcript;

use crate::action::ChatTuiAction;
use crate::state::State;

/// Applies one TUI action to state without performing IO or runtime side effects.
pub fn reduce(state: &mut State, action: ChatTuiAction) {
    match action {
        ChatTuiAction::PromptChanged(_)
        | ChatTuiAction::SubmitPrompt { .. }
        | ChatTuiAction::SelectionPromptChanged(_)
        | ChatTuiAction::SelectionPromptSubmitted(_)
        | ChatTuiAction::SelectionPromptCancelled
        | ChatTuiAction::CommandsLoaded(_)
        | ChatTuiAction::SessionChanged { .. }
        | ChatTuiAction::SessionCreated { .. }
        | ChatTuiAction::Resize { .. } => prompt::reduce(state, action),
        ChatTuiAction::MessageStarted { .. }
        | ChatTuiAction::MessageDelta { .. }
        | ChatTuiAction::MessageFinished { .. }
        | ChatTuiAction::ReasoningStarted { .. }
        | ChatTuiAction::ReasoningDelta { .. }
        | ChatTuiAction::ReasoningFinished { .. }
        | ChatTuiAction::WorkedSummaryReported { .. }
        | ChatTuiAction::ErrorReported { .. }
        | ChatTuiAction::WarningReported { .. }
        | ChatTuiAction::SuccessReported { .. }
        | ChatTuiAction::NoticeReported { .. } => transcript::reduce(state, action),
        ChatTuiAction::ToolCallStarted { .. }
        | ChatTuiAction::ToolCallDelta { .. }
        | ChatTuiAction::ToolCallFinished { .. }
        | ChatTuiAction::ToolCallFailed { .. }
        | ChatTuiAction::ToolDisplayStarted { .. }
        | ChatTuiAction::ToolDisplayFinished { .. }
        | ChatTuiAction::CommandStarted { .. }
        | ChatTuiAction::CommandOutput { .. }
        | ChatTuiAction::CommandFinished { .. }
        | ChatTuiAction::CommandDisplayStarted { .. }
        | ChatTuiAction::CommandDisplayOutput { .. }
        | ChatTuiAction::CommandDisplayFinished { .. } => display::reduce(state, action),
        ChatTuiAction::ExitRequested
        | ChatTuiAction::CancelRun
        | ChatTuiAction::AgentStarted
        | ChatTuiAction::AgentFinished
        | ChatTuiAction::AgentFailed { .. }
        | ChatTuiAction::AgentCancelled { .. }
        | ChatTuiAction::InputNoticeReported { .. }
        | ChatTuiAction::InputNoticeCleared
        | ChatTuiAction::RuntimeSelectionChanged(_)
        | ChatTuiAction::DisplayMetadataChanged(_)
        | ChatTuiAction::WorktreeMetadataChanged(_)
        | ChatTuiAction::ContextUsageUpdated(_)
        | ChatTuiAction::ProviderUsageReported(_)
        | ChatTuiAction::SpinnerTick => status::reduce(state, action),
    }
}
