use crate::ids::SessionId;
use crate::metadata::{CommandDescriptor, ContextTokenUsage, DisplayMetadata, RuntimeSelection};
use crate::scroll::TranscriptScrollState;
use crate::session::{SelectionPromptState, Session};
use crate::spinner::SpinnerState;
use crate::status::Status;

/// Complete framework-independent state for the full terminal UI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct State {
    pub session: Session,
    pub commands: Vec<CommandDescriptor>,
    pub runtime: RuntimeSelection,
    pub display: DisplayMetadata,
    pub status: Status,
    pub spinner: SpinnerState,
    pub selection: Option<SelectionPromptState>,
    pub scroll: TranscriptScrollState,
}

impl State {
    /// Creates initial TUI state from caller-owned runtime and display metadata.
    pub fn new(
        session_id: SessionId,
        runtime: RuntimeSelection,
        mut display: DisplayMetadata,
    ) -> Self {
        default_display_context_usage(&runtime, &mut display);
        Self {
            session: Session::new(session_id),
            commands: Vec::new(),
            runtime,
            display,
            status: Status::Idle,
            spinner: SpinnerState::new(),
            selection: None,
            scroll: TranscriptScrollState::follow_tail(),
        }
    }

    /// Reconstructs live TUI state from a durable session snapshot and fresh metadata.
    pub fn from_session(
        mut session: Session,
        commands: Vec<CommandDescriptor>,
        runtime: RuntimeSelection,
        mut display: DisplayMetadata,
    ) -> Self {
        session.refresh_next_timestamp();
        display.context_usage = session.context_usage;
        display.turn_usage = session.turn_usage;
        display.total_usage = session.total_usage;
        default_display_context_usage(&runtime, &mut display);
        Self {
            session,
            commands,
            runtime,
            display,
            status: Status::Idle,
            spinner: SpinnerState::new(),
            selection: None,
            scroll: TranscriptScrollState::follow_tail(),
        }
    }
}

/// Populates display context usage from the runtime context window when no context estimate exists.
pub(crate) fn default_display_context_usage(
    runtime: &RuntimeSelection,
    display: &mut DisplayMetadata,
) {
    if display.context_usage.is_some() {
        return;
    }

    display.context_usage = ContextTokenUsage::default_for_window(runtime.context_window_tokens);
}
