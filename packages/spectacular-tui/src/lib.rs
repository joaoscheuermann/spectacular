pub mod action;
pub mod components;
pub mod event_loop;
pub mod fake_streaming;
pub mod ids;
pub mod metadata;
pub mod reducer;
pub mod render;
pub mod runtime_shell;
pub mod scroll;
pub mod session;
pub mod spinner;
pub mod state;
pub mod status;
pub mod transcript;
mod transcript_window;

pub use action::ChatTuiAction;
pub use event_loop::{
    tui_event_effects, tui_timer_tick_effects, EventEffect, TUI_SPINNER_TICK_INTERVAL,
};
pub use fake_streaming::{
    fake_cancellation_plan, fake_failure_plan, fake_streaming_plan, fake_streaming_runtime_finding,
    FakeStreamingPlan, FakeStreamingTickOutcome, FakeStreamingTimeline,
};
pub use ids::{SessionId, Timestamp, TranscriptItemId};
pub use metadata::{
    CommandDescriptor, ContextTokenUsage, DisplayMetadata, ReasoningLevel, RuntimeSelection,
};
pub use reducer::reduce;
pub use render::render_state_to_string;
pub use runtime_shell::{RuntimeIntent, RuntimeShell};
pub use scroll::TranscriptScrollState;
pub use session::{PromptPasteBurstState, PromptState, SelectionPromptState, Session};
pub use spinner::SpinnerState;
pub use state::State;
pub use status::{Activity, Status};
pub use transcript::{
    AssistantMessageItem, CommandItem, CommandStatus, ErrorItem, NoticeItem, ReasoningItem,
    ToolCallItem, ToolStatus, TranscriptItem, TranscriptItemContent, UserPromptItem,
};

use anstyle::{RgbColor, Style};

const TEXT: RgbColor = RgbColor(229, 231, 235);
const DIM: RgbColor = RgbColor(148, 163, 184);
const COMMAND_OUTPUT: RgbColor = RgbColor(107, 114, 128);
const GREEN: RgbColor = RgbColor(34, 197, 94);
const YELLOW: RgbColor = RgbColor(234, 179, 8);
const RED: RgbColor = RgbColor(248, 113, 113);
const MAGENTA: RgbColor = RgbColor(217, 70, 239);
const CYAN: RgbColor = RgbColor(34, 211, 238);
const BLUE: RgbColor = RgbColor(96, 165, 250);
const ORANGE: RgbColor = RgbColor(251, 191, 36);
const SELECTION_BACKGROUND: RgbColor = RgbColor(51, 65, 85);

/// Applies a terminal style and reset sequence around display text.
pub fn paint(style: Style, value: impl AsRef<str>) -> String {
    let value = value.as_ref();
    format!("{style}{value}{style:#}")
}

/// Returns the default high-contrast style for primary terminal text.
pub fn text_style() -> Style {
    TEXT.on_default()
}

/// Returns the muted style for secondary labels, hints, and metadata.
pub fn dim_style() -> Style {
    DIM.on_default()
}

/// Returns the dark gray style for command output blocks.
pub fn command_output_style() -> Style {
    COMMAND_OUTPUT.on_default()
}

/// Returns the bold blue style for command invocation lines.
pub fn command_style() -> Style {
    BLUE.on_default().bold()
}

/// Returns the bold green style for successful status messages.
pub fn success_style() -> Style {
    GREEN.on_default().bold()
}

/// Returns the bold yellow style for warnings and cancellation notices.
pub fn warning_style() -> Style {
    YELLOW.on_default().bold()
}

/// Returns the bold red style for error messages.
pub fn error_style() -> Style {
    RED.on_default().bold()
}

/// Returns the green style for user-authored prompt text.
pub fn user_style() -> Style {
    GREEN.on_default()
}

/// Returns the primary text style for assistant responses.
pub fn assistant_style() -> Style {
    text_style()
}

/// Returns the bold magenta style for tool names and task-like labels.
pub fn tool_style() -> Style {
    MAGENTA.on_default().bold()
}

/// Returns the bold green style for product and section titles.
pub fn title_style() -> Style {
    GREEN.on_default().bold()
}

/// Returns the bold cyan style for provider names.
pub fn provider_style() -> Style {
    CYAN.on_default().bold()
}

/// Returns the bold magenta style for task names.
pub fn task_style() -> Style {
    MAGENTA.on_default().bold()
}

/// Returns the primary text style for model identifiers.
pub fn model_style() -> Style {
    text_style()
}

/// Returns the orange style for masked or secret-adjacent values.
pub fn secret_style() -> Style {
    ORANGE.on_default()
}

/// Returns the blue style for low-effort reasoning indicators.
pub fn low_reasoning_style() -> Style {
    BLUE.on_default()
}

/// Returns the inverse selection style for highlighted prompt text.
pub fn selection_style() -> Style {
    TEXT.on(SELECTION_BACKGROUND)
}

/// Returns the green foreground style for added diff text.
pub fn diff_added_style() -> Style {
    GREEN.on_default()
}

/// Returns the red foreground style for removed diff text.
pub fn diff_removed_style() -> Style {
    RED.on_default()
}

/// Formats compatibility display parts with terminal styles.
pub fn tool_line(label: &str, input: &str, metadata: Option<&str>) -> String {
    let mut output = Vec::new();
    push_styled_if_visible(&mut output, tool_style(), label);
    push_styled_if_visible(&mut output, text_style(), input);
    if let Some(metadata) = metadata {
        push_styled_if_visible(&mut output, dim_style(), metadata);
    }
    output.join(" ")
}

/// Formats a one-label/one-argument tool-call line.
pub fn tool_arg_line(label: &str, input: &str) -> String {
    tool_line(label, input, None)
}

/// Formats a two-label/two-argument tool-call line.
pub fn tool_arg_tool_arg_line(
    label: &str,
    first_argument: &str,
    second_label: &str,
    second_argument: &str,
) -> String {
    let mut output = Vec::new();
    push_styled_if_visible(&mut output, tool_style(), label);
    push_styled_if_visible(&mut output, text_style(), first_argument);
    push_styled_if_visible(&mut output, tool_style(), second_label);
    push_styled_if_visible(&mut output, text_style(), second_argument);
    output.join(" ")
}

/// Adds a styled segment when the raw text has visible content.
pub fn push_styled_if_visible(output: &mut Vec<String>, style: Style, value: &str) {
    if value.trim().is_empty() {
        return;
    }

    output.push(paint(style, value));
}
