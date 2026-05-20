use crate::render_model::{iocraft_content, RenderLine, RenderStyle};
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

    let Some(line) = working_render_line(&state) else {
        return element!(View(width: 100pct)).into_any();
    };

    let contents = iocraft_content(&line);
    element!(View(width: 100pct, margin_bottom: 1) {
        MixedText(wrap: TextWrap::NoWrap, contents)
    })
    .into_any()
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
