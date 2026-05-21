use crate::render::{iocraft_content, RenderLine, RenderStyle};
use crate::runtime::SPINNER_TICK_INTERVAL;
use crate::spinner::SpinnerState;
use crate::state::State;
use crate::status::Status;
use iocraft::prelude::*;

/// Renders the active working indicator row when a request is in flight.
#[component]
pub fn Working(mut hooks: Hooks, props: &WorkingProps) -> impl Into<AnyElement<'static>> {
    let mut spinner = hooks.use_state(SpinnerState::new);
    let frame = { spinner.read().current_frame() };
    hooks.use_future(async move {
        loop {
            tokio::time::sleep(SPINNER_TICK_INTERVAL).await;
            let mut next = spinner.read().clone();
            next.tick();
            spinner.set(next);
        }
    });

    let state = props.state.clone().expect("Working requires state");

    let Some(line) = working_render_line_with_frame(&state, frame) else {
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
    working_render_line_with_frame(state, state.spinner.current_frame())
}

/// Formats the working row with the caller-owned spinner frame.
fn working_render_line_with_frame(state: &State, frame: &str) -> Option<RenderLine> {
    match &state.status {
        Status::Running { .. } | Status::Cancelling => Some(RenderLine::styled(
            format!("{frame} Working (CTRL + C to stop)"),
            RenderStyle::Dim,
        )),
        Status::Idle | Status::Failed { .. } => None,
    }
}

/// Props for the working component.
#[derive(Default, Props)]
pub struct WorkingProps {
    pub state: Option<State>,
}
