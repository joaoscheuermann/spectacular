use crate::components::transcript_content::render_line_element;
use crate::render_model::{RenderLine, RenderStyle};
use crate::state::State;
use crate::status::Status;
use iocraft::prelude::*;

/// Renders the active working indicator row when a request is in flight.
#[component]
pub fn WorkingIndicator(props: &WorkingIndicatorProps) -> impl Into<AnyElement<'static>> {
    let state = props
        .state
        .clone()
        .expect("WorkingIndicator requires state");

    let lines: Vec<AnyElement<'static>> = working_render_line(&state)
        .into_iter()
        .map(render_line_element)
        .collect();

    element!(View(width: 100pct) { #(lines.into_iter()) })
}

/// Formats the current working status line as a semantic row when active.
pub fn working_render_line(state: &State) -> Option<RenderLine> {
    match &state.status {
        Status::Running { .. } | Status::Cancelling => Some(RenderLine::styled(
            format!(
                "{} Working (CTRL + C to stop)",
                state.spinner.current_frame()
            ),
            RenderStyle::Dim,
        )),
        Status::Idle | Status::Failed { .. } => None,
    }
}

/// Props for the working-indicator component.
#[derive(Default, Props)]
pub struct WorkingIndicatorProps {
    pub state: Option<State>,
}
