use crate::format::app_lines;
use crate::state::State;
use iocraft::prelude::*;

const PREVIEW_RENDER_HEIGHT: u16 = 240;

/// Renders the IOCraft application layout to plain text for tests and previews.
pub fn render_state_to_string(state: &State, max_width: Option<usize>) -> String {
    element!(crate::components::App(
        state: state.clone(),
        width: max_width.and_then(|width| u16::try_from(width).ok()),
        height: Some(PREVIEW_RENDER_HEIGHT)
    ))
    .render(max_width);

    app_lines(state).join("\n")
}
