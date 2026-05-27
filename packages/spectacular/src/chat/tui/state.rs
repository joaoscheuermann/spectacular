use crate::chat::model::ChatModel;
use crate::chat::worktree::current_worktree_metadata;
use crate::chat::RuntimeSelection;
use spectacular_agent::ContextTokenUsage;
use spectacular_tui::{
    ChatTuiAction, DisplayMetadata, OpeningBannerItem, ReasoningLevel, SessionId, State,
};
use std::path::Path;

/// Builds initial TUI state from chat model metadata without direct terminal writes.
pub(crate) fn initial(model: &ChatModel, workspace_root: &Path) -> State {
    let mut display = display_metadata(
        model.current_session_id(),
        model.runtime(),
        workspace_root,
        model.context_token_usage(),
    );
    display.worktree = current_worktree_metadata(workspace_root);
    State::new(
        SessionId::new(model.current_session_id()),
        runtime_selection(model.runtime()),
        display,
    )
}

/// Builds the semantic opening banner action for a newly created TUI session.
pub(crate) fn session_created_action(
    id: &str,
    model: &ChatModel,
    workspace_root: &Path,
) -> ChatTuiAction {
    ChatTuiAction::SessionCreated {
        id: SessionId::new(id),
        banner: OpeningBannerItem::new(
            env!("CARGO_PKG_VERSION"),
            model.runtime().model.clone(),
            model.runtime().reasoning.to_string(),
            workspace_root.to_string_lossy(),
            id,
        ),
    }
}

/// Projects runtime selection into the UI-safe TUI runtime model.
pub(crate) fn runtime_selection(runtime: &RuntimeSelection) -> spectacular_tui::RuntimeSelection {
    spectacular_tui::RuntimeSelection::new(
        runtime.provider_type.clone(),
        runtime.provider.clone(),
        runtime.model.clone(),
        reasoning_level(runtime.reasoning),
        runtime.context_window_tokens.map(|value| value as u64),
    )
}

/// Projects runtime selection into a reducer action.
#[cfg(test)]
pub(crate) fn runtime_selection_action(runtime: &RuntimeSelection) -> ChatTuiAction {
    ChatTuiAction::RuntimeSelectionChanged(runtime_selection(runtime))
}

/// Projects header and footer metadata into UI-safe display state.
pub(crate) fn display_metadata(
    session_id: &str,
    runtime: &RuntimeSelection,
    current_directory: &Path,
    usage: Option<ContextTokenUsage>,
) -> DisplayMetadata {
    DisplayMetadata::new(
        runtime.provider.clone(),
        runtime.model.clone(),
        runtime.reasoning.to_string(),
        current_directory.to_string_lossy(),
        session_id,
        usage.map(context_usage),
    )
}

/// Projects header and footer metadata into a reducer action.
#[cfg(test)]
pub(crate) fn display_metadata_action(
    session_id: &str,
    runtime: &RuntimeSelection,
    current_directory: &Path,
    usage: Option<ContextTokenUsage>,
) -> ChatTuiAction {
    ChatTuiAction::DisplayMetadataChanged(display_metadata(
        session_id,
        runtime,
        current_directory,
        usage,
    ))
}

/// Converts runtime context token usage into TUI token usage metadata.
pub(crate) fn context_usage(usage: ContextTokenUsage) -> spectacular_tui::ContextTokenUsage {
    spectacular_tui::ContextTokenUsage::new(usage.input_tokens, usage.context_window_tokens)
}

/// Maps Spectacular runtime reasoning levels into the TUI display subset.
fn reasoning_level(reasoning: spectacular_config::ReasoningLevel) -> ReasoningLevel {
    match reasoning {
        spectacular_config::ReasoningLevel::None => ReasoningLevel::None,
        spectacular_config::ReasoningLevel::Minimal | spectacular_config::ReasoningLevel::Low => {
            ReasoningLevel::Low
        }
        spectacular_config::ReasoningLevel::Medium => ReasoningLevel::Medium,
        spectacular_config::ReasoningLevel::High | spectacular_config::ReasoningLevel::Xhigh => {
            ReasoningLevel::High
        }
    }
}
