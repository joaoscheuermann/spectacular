use crate::ids::SessionId;
use crate::metadata::{CommandDescriptor, DisplayMetadata, RuntimeSelection};
use crate::scroll::TranscriptScrollState;
use crate::session::{SelectionPromptState, Session};
use crate::spinner::SpinnerState;
use crate::status::Status;
use crate::streaming::AssistantStreamState;

/// Complete framework-independent state for the full terminal UI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct State {
    pub session: Session,
    pub commands: Vec<CommandDescriptor>,
    pub runtime: RuntimeSelection,
    pub display: DisplayMetadata,
    pub status: Status,
    pub spinner: SpinnerState,
    pub assistant_stream: AssistantStreamState,
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
            assistant_stream: AssistantStreamState::default(),
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
        display.usage = session.usage;
        Self {
            session,
            commands,
            runtime,
            display,
            status: Status::Idle,
            spinner: SpinnerState::new(),
            assistant_stream: AssistantStreamState::default(),
            selection: None,
            scroll: TranscriptScrollState::follow_tail(),
        }
    }
}
