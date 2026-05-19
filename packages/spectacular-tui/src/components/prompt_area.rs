use crate::components::transcript_content::render_line_element;
use crate::format::prompt_render_lines;
use crate::state::State;
use iocraft::prelude::*;

/// Renders prompt text, suggestions, and command guidance rows.
#[component]
pub fn PromptArea(props: &PromptAreaProps) -> impl Into<AnyElement<'static>> {
    let state = props.state.clone().expect("PromptArea requires state");
    let lines = prompt_render_lines(&state);

    element!(View(width: 100pct) {
        #(lines.into_iter().map(render_line_element))
    })
}

/// Props for the prompt-area component.
#[derive(Default, Props)]
pub struct PromptAreaProps {
    pub state: Option<State>,
}
