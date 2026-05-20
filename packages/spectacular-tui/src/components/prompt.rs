use crate::render::{iocraft_content, RenderLine};
use crate::state::State;
use iocraft::prelude::*;

/// Renders the active prompt rows and contextual suggestions.
#[component]
pub fn Prompt(props: &PromptProps) -> impl Into<AnyElement<'static>> {
    let state = props.state.clone().expect("Prompt requires state");
    let elements = prompt_render_lines_with_width(&state, props.width)
        .into_iter()
        .map(|line| {
            let contents = iocraft_content(&line);
            element!(MixedText(wrap: TextWrap::NoWrap, contents))
        });

    element!(View(flex_direction: FlexDirection::Column, width: 100pct, margin_bottom: 1) { #(elements) })
}

/// Formats the active prompt with default width for compatibility tests.
pub fn prompt_render_lines(state: &State) -> Vec<RenderLine> {
    prompt_render_lines_with_width(state, None)
}

/// Formats the active prompt with width-aware wrapping.
pub fn prompt_render_lines_with_width(state: &State, width: Option<u16>) -> Vec<RenderLine> {
    crate::prompt::render_lines(&state.session.prompt, &state.commands, width)
}

/// Formats the active prompt as plain visible text for compatibility tests.
pub fn prompt_lines(state: &State) -> Vec<String> {
    crate::components::plain_lines(prompt_render_lines(state))
}

/// Props for the prompt component.
#[derive(Default, Props)]
pub struct PromptProps {
    pub state: Option<State>,
    pub width: Option<u16>,
}
