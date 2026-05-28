use tui::components::app_render_lines;
use tui::{
    reduce, ChatTuiAction, CommandDisplayChunk, CommandDisplayStatus, DisplayLine,
    DisplayLineStyle, DisplayMetadata, ReasoningLevel, RuntimeSelection, Session, SessionId, State,
    ToolDisplayStatus, TranscriptItemContent, TranscriptItemId,
};

/// Builds representative runtime metadata for tool and command parity tests.
fn runtime() -> RuntimeSelection {
    RuntimeSelection::new(
        "openai-compatible",
        "provider",
        "model",
        ReasoningLevel::Low,
        Some(4096),
    )
}

/// Builds display metadata for tool and command parity tests.
fn display() -> DisplayMetadata {
    DisplayMetadata::new("provider", "model", "low", "/workspace", "session", None)
}

/// Builds initialized TUI state for tool and command parity tests.
fn state() -> State {
    State::new(SessionId::new("session-1"), runtime(), display())
}

/// Builds a stable transcript item id.
fn id(value: &str) -> TranscriptItemId {
    TranscriptItemId::new(value)
}

/// Builds one display line for action payloads.
fn line(text: &str, style: DisplayLineStyle) -> DisplayLine {
    DisplayLine::new(text, style)
}

/// Returns the current render model as plain text and semantic row style pairs.
fn rendered_lines(state: &State) -> Vec<(String, DisplayLineStyle)> {
    app_render_lines(state)
        .into_iter()
        .filter_map(|line| {
            let span = line.spans.first()?;
            Some((line.plain_text(), DisplayLineStyle::from(span.style)))
        })
        .collect()
}

#[path = "tool_command_adapter_parity/command_display.rs"]
mod command_display;
#[path = "tool_command_adapter_parity/snapshot.rs"]
mod snapshot;
#[path = "tool_command_adapter_parity/tool_display.rs"]
mod tool_display;
