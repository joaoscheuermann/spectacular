use crate::state::State;
use iocraft::prelude::*;

/// Renders the read-only IOCraft prototype layout to plain text for tests and previews.
pub fn render_state_to_string(state: &State, max_width: Option<usize>) -> String {
    element!(crate::components::App(state))
        .render(max_width)
        .to_string()
}
