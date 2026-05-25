use spectacular_tui::components::app_render_lines;
use spectacular_tui::{
    footer_render_line, footer_right_render_line, render_state_to_string, CommandStatus,
    ContextTokenUsage, DisplayMetadata, OpeningBannerItem, ReasoningLevel, RenderLine, RenderStyle,
    RuntimeSelection, SessionId, State, Status, ToolStatus, TranscriptItem, TranscriptItemContent,
    TranscriptItemId, TurnTokenUsage, WorktreeMetadata,
};

/// Builds a representative runtime selection for active render parity tests.
fn runtime() -> RuntimeSelection {
    RuntimeSelection::new(
        "openai-compatible",
        "openrouter",
        "gpt-5.1",
        ReasoningLevel::High,
        Some(200_000),
    )
}

/// Builds display metadata for active render parity tests.
fn display(usage: Option<ContextTokenUsage>) -> DisplayMetadata {
    DisplayMetadata::new(
        "OpenRouter",
        "GPT 5.1",
        "high",
        "/workspace/spectacular",
        "session-123",
        usage,
    )
}

/// Builds an initialized state with stable metadata.
fn state() -> State {
    State::new(
        SessionId::new("session-123"),
        runtime_without_context_window(),
        display(None),
    )
}

/// Builds a runtime selection without context usage for legacy no-usage checks.
fn runtime_without_context_window() -> RuntimeSelection {
    RuntimeSelection::new(
        "openai-compatible",
        "openrouter",
        "gpt-5.1",
        ReasoningLevel::High,
        None,
    )
}

/// Creates a transcript item with stable test identity and timestamp.
fn item(index: u64, content: TranscriptItemContent) -> TranscriptItem {
    TranscriptItem::new(
        TranscriptItemId::new(format!("item-{index}")),
        spectacular_tui::Timestamp::new(index),
        content,
    )
}

/// Renders state through the active IOCraft application path.
fn render(state: &State) -> String {
    render_state_to_string(state, Some(100))
}

/// Returns the visible text from semantic render lines.
fn visible_text(lines: &[RenderLine]) -> Vec<String> {
    lines.iter().map(RenderLine::plain_text).collect()
}

#[path = "active_render_visual_parity/chrome.rs"]
mod chrome;
#[path = "active_render_visual_parity/footer.rs"]
mod footer;
#[path = "active_render_visual_parity/transcript.rs"]
mod transcript;
