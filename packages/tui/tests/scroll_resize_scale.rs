use crossterm::event::MouseButton;
use iocraft::prelude::{FullscreenMouseEvent, MouseEventKind, TerminalEvent};
use std::time::{Duration, Instant};
use tui::{
    reduce, render_state_to_string, AssistantMessageItem, ChatTuiAction, DisplayMetadata,
    ReasoningLevel, RuntimeSelection, SelectableProjection, SelectableSurface, SessionId, State,
    TranscriptItem, TranscriptItemContent, TranscriptItemId, UserPromptItem,
};

const LARGE_TRANSCRIPT_ITEMS: usize = 20_000;
const VISIBLE_TRANSCRIPT_ROWS: u16 = 20;
const RENDER_BUDGET: Duration = Duration::from_secs(2);
const STREAMING_DRAG_BUDGET: Duration = Duration::from_secs(3);

/// Builds a representative runtime selection for scroll and scale tests.
fn runtime() -> RuntimeSelection {
    RuntimeSelection::new(
        "openai-compatible",
        "provider",
        "model",
        ReasoningLevel::Low,
        Some(4096),
    )
}

/// Builds visible display metadata for scroll and scale tests.
fn display() -> DisplayMetadata {
    DisplayMetadata::new("provider", "model", "low", "/workspace", "session", None)
}

/// Builds an initialized state with stable metadata and viewport size.
fn state() -> State {
    let mut state = State::new(SessionId::new("session-1"), runtime(), display());
    state.scroll.visible_rows = u32::from(VISIBLE_TRANSCRIPT_ROWS);
    state
}

/// Builds a stable transcript item ID for scale fixtures.
fn item_id(index: usize) -> TranscriptItemId {
    TranscriptItemId::new(format!("item-{index}"))
}

/// Builds a fullscreen mouse terminal event.
fn mouse(kind: MouseEventKind, column: u16, row: u16) -> TerminalEvent {
    TerminalEvent::FullscreenMouse(FullscreenMouseEvent::new(kind, column, row))
}

/// Builds a semantic transcript item with stable identity, timestamp, and visible text.
fn transcript_item(index: usize) -> TranscriptItem {
    let content = TranscriptItemContent::UserPrompt(UserPromptItem::new(format!(
        "large transcript item {index}"
    )));
    TranscriptItem::new(item_id(index), tui::Timestamp::new(index as u64), content)
}

/// Populates state with a deterministic large semantic transcript fixture.
fn populate_large_transcript(state: &mut State) {
    state.session.transcript = (0..LARGE_TRANSCRIPT_ITEMS).map(transcript_item).collect();
}

/// Renders state and returns the output with elapsed render time.
fn timed_render(state: &State) -> (String, Duration) {
    let started = Instant::now();
    let output = render_state_to_string(state, Some(120));
    (output, started.elapsed())
}

#[path = "scroll_resize_scale/bounded_window.rs"]
mod bounded_window;
#[path = "scroll_resize_scale/layout_growth.rs"]
mod layout_growth;
#[path = "scroll_resize_scale/state.rs"]
mod scroll_state;
#[path = "scroll_resize_scale/streaming.rs"]
mod streaming;
