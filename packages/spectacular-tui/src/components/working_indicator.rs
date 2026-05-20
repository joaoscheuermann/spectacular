use crate::components::transcript_content::render_line_element;
use crate::format::working_render_line;
use crate::state::State;
use iocraft::prelude::*;

/// Renders the active working indicator row when the app is busy.
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

/// Props for the working-indicator component.
#[derive(Default, Props)]
pub struct WorkingIndicatorProps {
    pub state: Option<State>,
}
