mod ansi;
mod directory;
mod model;

use crate::components::app_lines;
use crate::state::State;

pub use ansi::display_spans_from_ansi;
pub(crate) use directory::format_directory;
pub use directory::format_directory_with_home;
pub use model::{
    context_pressure_style, context_usage_style, iocraft_content,
    iocraft_content_with_selection_colors, semantic_ansi_style, semantic_iocraft_style,
    semantic_iocraft_style_with_selection_colors, RenderHighlight, RenderLine, RenderSpan,
    RenderStyle, TuiRgb, TuiSelectionColors, TUI_SELECTION_BACKGROUND_COLOR_ENV,
    TUI_SELECTION_TEXT_COLOR_ENV,
};

/// Renders the bounded semantic application projection to plain text for tests and previews.
pub fn render_state_to_string(state: &State, _max_width: Option<usize>) -> String {
    app_lines(state).join("\n")
}
