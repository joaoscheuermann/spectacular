use crate::components::footer::Footer;
use crate::components::prompt_area::PromptArea;
use crate::components::transcript::Transcript;
use crate::components::working_indicator::WorkingIndicator;
use crate::format::{prompt_render_lines, working_render_line};
use crate::state::State;
use iocraft::prelude::*;

/// Composes the full-screen application layout from owned state for runtime rendering.
#[component]
pub fn App(mut hooks: Hooks, props: &AppProps) -> impl Into<AnyElement<'static>> {
    let (terminal_width, terminal_height) = hooks.use_terminal_size();

    let width = props.width.unwrap_or(terminal_width);
    let height = props.height.or_else(|| non_zero_size(terminal_height));

    let state = props.state.clone().expect("App requires state");

    let Some(height) = height else {
        return element!(View(width)).into_any();
    };

    let transcript_capacity = transcript_capacity_rows(&state, height);

    element!(View(flex_direction: FlexDirection::Column, width, height) {
        Transcript(state: state.clone(), capacity: transcript_capacity)
        WorkingIndicator(state: state.clone())
        View(flex_direction: FlexDirection::Column, width: 100pct, flex_shrink: 0.0) {
            PromptArea(state: state.clone())
            Footer(state: state.clone())
        }
    })
    .into_any()
}

/// Returns rows available to transcript content after fixed chrome is accounted for.
fn transcript_capacity_rows(state: &State, height: u16) -> u16 {
    let working_rows = if working_render_line(state).is_some() {
        1
    } else {
        0
    };
    let chrome_rows = prompt_render_lines(state)
        .len()
        .saturating_add(working_rows)
        .saturating_add(1);
    let chrome_rows = u16::try_from(chrome_rows).unwrap_or(u16::MAX);

    height.saturating_sub(chrome_rows)
}

/// Props for the full-screen root application component.
#[derive(Default, Props)]
pub struct AppProps {
    pub state: Option<State>,
    pub width: Option<u16>,
    pub height: Option<u16>,
}

/// Returns a terminal size only when IOCraft has observed a real non-zero dimension.
fn non_zero_size(size: u16) -> Option<u16> {
    if size == 0 {
        return None;
    }

    Some(size)
}
