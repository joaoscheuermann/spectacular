use crate::terminal_style;
use anstyle::Style;

/// Applies a terminal style to display text.
pub(crate) fn paint(style: Style, value: impl AsRef<str>) -> String {
    terminal_style::paint(style, value)
}

/// Returns the style used for user-authored prompt text.
pub(crate) fn user_style() -> Style {
    terminal_style::user_style()
}

/// Returns the style used for assistant output.
pub(super) fn assistant_style() -> Style {
    terminal_style::assistant_style()
}

/// Returns the style used for tool names and tool payload labels.
pub(super) fn tool_style() -> Style {
    terminal_style::tool_style()
}

/// Returns the style used for successful status messages.
pub(super) fn success_style() -> Style {
    terminal_style::success_style()
}

/// Returns the style used for warnings and cancellation notices.
pub(super) fn warning_style() -> Style {
    terminal_style::warning_style()
}

/// Returns the style used for errors.
pub(super) fn error_style() -> Style {
    terminal_style::error_style()
}

/// Returns the style used for dimmed secondary text.
pub(crate) fn dim_style() -> Style {
    terminal_style::dim_style()
}

/// Returns the style used for selected prompt text.
pub(crate) fn selection_style() -> Style {
    terminal_style::selection_style()
}
