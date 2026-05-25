use spectacular_tui::{ChatTuiAction, SessionId, TranscriptItemId};

/// Builds the TUI action for a controller-owned agent run start.
pub(crate) fn agent_started_action() -> ChatTuiAction {
    ChatTuiAction::AgentStarted
}

/// Builds the TUI action for a submitted prompt with caller-owned identity.
pub(crate) fn submit_prompt_action(
    id: impl Into<String>,
    text: impl Into<String>,
) -> ChatTuiAction {
    ChatTuiAction::SubmitPrompt {
        id: TranscriptItemId::new(id),
        text: text.into(),
    }
}

/// Builds the TUI action for switching to another session state root.
pub(crate) fn session_changed_action(session_id: &str) -> ChatTuiAction {
    ChatTuiAction::SessionChanged {
        id: SessionId::new(session_id),
    }
}
