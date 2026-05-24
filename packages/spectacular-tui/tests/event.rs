use crossterm::event::MouseButton;
use iocraft::prelude::{
    FullscreenMouseEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind,
    TerminalEvent,
};
use spectacular_tui::{
    effects, effects_with_clipboard, reduce, ChatTuiAction, ClipboardError, ClipboardService,
    CommandDescriptor, DisplayMetadata, EventEffect, PromptLayoutMetrics, PromptState,
    ReasoningLevel, RuntimeSelection, SelectableProjection, SelectableSurface,
    SelectionPromptState, SessionId, State, Status, TranscriptItemContent, TranscriptItemId,
    ViewAction, COPIED_SELECTION_NOTICE, MAX_PASTE_BYTES, SPINNER_TICK_INTERVAL,
};
use std::collections::VecDeque;
use std::time::Duration;

/// Builds a representative runtime selection for event-loop tests.
fn runtime() -> RuntimeSelection {
    RuntimeSelection::new(
        "openai-compatible",
        "provider",
        "model",
        ReasoningLevel::Low,
        Some(4096),
    )
}

/// Builds visible display metadata for event-loop tests.
fn display() -> DisplayMetadata {
    DisplayMetadata::new("provider", "model", "low", "/workspace", "session", None)
}

/// Builds an initialized state with stable metadata.
fn state() -> State {
    State::new(SessionId::new("session-1"), runtime(), display())
}

/// Builds a key terminal event with optional modifier flags.
fn key(code: KeyCode, modifiers: KeyModifiers) -> TerminalEvent {
    let mut event = KeyEvent::new(KeyEventKind::Press, code);
    event.modifiers = modifiers;
    TerminalEvent::Key(event)
}

/// Builds a fullscreen mouse terminal event.
fn mouse(kind: MouseEventKind, column: u16, row: u16) -> TerminalEvent {
    TerminalEvent::FullscreenMouse(FullscreenMouseEvent::new(kind, column, row))
}

#[derive(Default)]
struct FakeClipboard {
    reads: VecDeque<String>,
    writes: Vec<String>,
    read_error: bool,
    write_error: bool,
}

impl FakeClipboard {
    /// Creates a fake clipboard that returns one text value when pasted.
    fn with_text(text: &str) -> Self {
        Self {
            reads: VecDeque::from([text.to_owned()]),
            writes: Vec::new(),
            read_error: false,
            write_error: false,
        }
    }

    /// Creates a fake clipboard that fails when text is read.
    fn failing_read() -> Self {
        Self {
            read_error: true,
            ..Self::default()
        }
    }

    /// Creates a fake clipboard that fails when text is written.
    fn failing_write() -> Self {
        Self {
            write_error: true,
            ..Self::default()
        }
    }
}

impl ClipboardService for FakeClipboard {
    /// Reads the next queued fake clipboard value.
    fn get_text(&mut self) -> Result<String, ClipboardError> {
        if self.read_error {
            return Err(ClipboardError::message("read failed"));
        }

        Ok(self.reads.pop_front().unwrap_or_default())
    }

    /// Records text written by copy or cut.
    fn set_text(&mut self, text: &str) -> Result<(), ClipboardError> {
        if self.write_error {
            return Err(ClipboardError::message("write failed"));
        }

        self.writes.push(text.to_owned());
        Ok(())
    }
}

/// Extracts the single action produced by one terminal event.
fn single_action(state: &State, event: TerminalEvent) -> ChatTuiAction {
    let effects = effects(state, event);
    action_from_effects(effects)
}

/// Extracts the single action produced using a fake clipboard service.
fn single_action_with_clipboard(
    state: &State,
    event: TerminalEvent,
    clipboard: &mut dyn ClipboardService,
) -> ChatTuiAction {
    let effects = effects_with_clipboard(state, event, Some(clipboard));
    action_from_effects(effects)
}

/// Extracts the single view action produced using a fake clipboard service.
fn single_view_action_with_clipboard(
    state: &State,
    event: TerminalEvent,
    clipboard: &mut dyn ClipboardService,
) -> ViewAction {
    let effects = effects_with_clipboard(state, event, Some(clipboard));
    view_action_from_effects(effects)
}

/// Extracts the single view action produced by one terminal event.
fn single_view_action(state: &State, event: TerminalEvent) -> ViewAction {
    let effects = effects(state, event);
    view_action_from_effects(effects)
}

/// Extracts the reducer action from one event effect.
fn action_from_effects(effects: Vec<EventEffect>) -> ChatTuiAction {
    assert_eq!(effects.len(), 1);
    match effects.into_iter().next().unwrap() {
        EventEffect::Action(action) => *action,
        EventEffect::ViewAction(_) => panic!("expected semantic action effect"),
        EventEffect::RequestExit => panic!("expected action effect"),
    }
}

/// Extracts the view action from one event effect.
fn view_action_from_effects(effects: Vec<EventEffect>) -> ViewAction {
    assert_eq!(effects.len(), 1);
    match effects.into_iter().next().unwrap() {
        EventEffect::ViewAction(action) => *action,
        EventEffect::Action(_) => panic!("expected view action effect"),
        EventEffect::RequestExit => panic!("expected view action effect"),
    }
}

// Verifies typed characters are translated into reducer-owned prompt state updates.

#[path = "event/event_clipboard.rs"]
mod event_clipboard;
#[path = "event/event_prompt.rs"]
mod event_prompt;
#[path = "event/event_rendered_selection.rs"]
mod event_rendered_selection;
#[path = "event/event_system.rs"]
mod event_system;
