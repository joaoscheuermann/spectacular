use tui::components::app_render_lines;
use tui::{
    reduce, ChatTuiAction, CommandStatus, DisplayMetadata, ReasoningLevel, RenderStyle,
    RuntimeSelection, SessionId, State, ToolStatus, TranscriptItemContent, TranscriptItemId,
};

/// Builds representative runtime metadata for streaming parity tests.
fn runtime() -> RuntimeSelection {
    RuntimeSelection::new(
        "openai-compatible",
        "provider",
        "model",
        ReasoningLevel::Low,
        Some(4096),
    )
}

/// Builds display metadata for streaming parity tests.
fn display() -> DisplayMetadata {
    DisplayMetadata::new("provider", "model", "low", "/workspace", "session", None)
}

/// Builds initialized TUI state for streaming parity tests.
fn state() -> State {
    State::new(SessionId::new("session-1"), runtime(), display())
}

/// Builds a stable transcript item id.
fn id(value: &str) -> TranscriptItemId {
    TranscriptItemId::new(value)
}

/// Returns the assistant item text currently visible to the renderer.
fn assistant_text(state: &State) -> &str {
    let TranscriptItemContent::AssistantMessage(item) = &state.session.transcript[0].content else {
        panic!("expected assistant item");
    };
    &item.text
}

#[path = "streaming_transcript_parity/assistant.rs"]
mod assistant;
#[path = "streaming_transcript_parity/scroll_follow.rs"]
mod scroll_follow;
#[path = "streaming_transcript_parity/semantic_items.rs"]
mod semantic_items;
