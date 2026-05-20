use crate::components::app_lines;
use crate::state::State;

/// Renders the bounded semantic application projection to plain text for tests and previews.
pub fn render_state_to_string(state: &State, _max_width: Option<usize>) -> String {
    app_lines(state).join("\n")
}
