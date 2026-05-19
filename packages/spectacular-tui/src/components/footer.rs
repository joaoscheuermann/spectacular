use crate::components::transcript_content::render_line_element;
use crate::format::footer_render_line;
use crate::state::State;
use iocraft::prelude::*;

/// Renders footer metadata from display/session state without external lookups.
#[component]
pub fn Footer(props: &FooterProps) -> impl Into<AnyElement<'static>> {
    let state = props.state.clone().expect("Footer requires state");
    let line = footer_render_line(&state);

    element!(View(width: 100pct) { #(vec![render_line_element(line)].into_iter()) })
}

/// Props for the footer component.
#[derive(Default, Props)]
pub struct FooterProps {
    pub state: Option<State>,
}
