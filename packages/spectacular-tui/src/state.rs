use crate::ids::SessionId;
use crate::metadata::{CommandDescriptor, DisplayMetadata, RuntimeSelection};
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
    pub fn new(session_id: SessionId, runtime: RuntimeSelection, display: DisplayMetadata) -> Self {
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
}
