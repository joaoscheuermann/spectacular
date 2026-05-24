use crate::ids::SessionId;
use crate::metadata::{CommandDescriptor, ContextTokenUsage, DisplayMetadata, RuntimeSelection};
use crate::render::TuiSelectionColors;
use crate::scroll::TranscriptScrollState;
use crate::selection::RenderedSelectionState;
use crate::session::{SelectionPromptState, Session};
use crate::spinner::SpinnerState;
use crate::status::Status;

const PROMPT_MARKER_WIDTH: u16 = 2;
const DEFAULT_PROMPT_CONTENT_WIDTH: usize = 118;
const DEFAULT_PROMPT_VIEWPORT_HEIGHT: usize = usize::MAX;

/// Current terminal-derived prompt textarea layout metrics used by reducer input handling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PromptLayoutMetrics {
    pub content_width: usize,
    pub viewport_height: usize,
}

impl PromptLayoutMetrics {
    /// Builds prompt layout metrics from the latest terminal size.
    pub fn from_terminal_size(width: u16, height: u16) -> Self {
        Self {
            content_width: width.saturating_sub(PROMPT_MARKER_WIDTH).max(1).into(),
            viewport_height: height.saturating_sub(2).max(1).into(),
        }
    }
}

impl Default for PromptLayoutMetrics {
    /// Creates unbounded prompt metrics for non-terminal tests and snapshots.
    fn default() -> Self {
        Self {
            content_width: DEFAULT_PROMPT_CONTENT_WIDTH,
            viewport_height: DEFAULT_PROMPT_VIEWPORT_HEIGHT,
        }
    }
}

/// Complete framework-independent state for the full terminal UI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct State {
    pub session: Session,
    pub commands: Vec<CommandDescriptor>,
    pub runtime: RuntimeSelection,
    pub display: DisplayMetadata,
    pub status: Status,
    pub exit_requested: bool,
    pub input_notice: Option<String>,
    pub spinner: SpinnerState,
    pub selection: Option<SelectionPromptState>,
    pub app_selection: RenderedSelectionState,
    pub scroll: TranscriptScrollState,
    pub prompt_layout: PromptLayoutMetrics,
    pub selection_colors: TuiSelectionColors,
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
            exit_requested: false,
            input_notice: None,
            spinner: SpinnerState::new(),
            selection: None,
            app_selection: RenderedSelectionState::default(),
            scroll: TranscriptScrollState::follow_tail(),
            prompt_layout: PromptLayoutMetrics::default(),
            selection_colors: TuiSelectionColors::from_env(),
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
            exit_requested: false,
            input_notice: None,
            spinner: SpinnerState::new(),
            selection: None,
            app_selection: RenderedSelectionState::default(),
            scroll: TranscriptScrollState::follow_tail(),
            prompt_layout: PromptLayoutMetrics::default(),
            selection_colors: TuiSelectionColors::from_env(),
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
