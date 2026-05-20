mod directory;
mod model;

use crate::components::app_lines;
use crate::state::State;

pub(crate) use directory::format_directory;
pub use directory::format_directory_with_home;
pub use model::{
    context_pressure_style, context_usage_style, iocraft_content, semantic_ansi_style,
    semantic_iocraft_style, RenderLine, RenderSpan, RenderStyle,
};

/// Renders the bounded semantic application projection to plain text for tests and previews.
pub fn render_state_to_string(state: &State, _max_width: Option<usize>) -> String {
    app_lines(state).join("\n")
}
