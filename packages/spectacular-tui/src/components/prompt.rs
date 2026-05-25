use crate::render::{iocraft_content_with_selection_colors, RenderLine};
use crate::selection::{style_line_for_source, SelectableSource};
use crate::state::State;
use iocraft::prelude::*;
use std::sync::Arc;

/// Renders the active prompt rows and contextual suggestions.
#[component]
pub fn Prompt(props: &PromptProps) -> impl Into<AnyElement<'static>> {
    let state = props.state.as_ref();
    let elements = prompt_rows(&state, props.width)
        .into_iter()
        .enumerate()
        .map(|(index, line)| {
            let line =
                style_line_for_source(&state, line, SelectableSource::Prompt { line: index });
            let contents = iocraft_content_with_selection_colors(&line, state.selection_colors);
            element!(MixedText(wrap: TextWrap::NoWrap, contents))
        });

    element!(View(flex_direction: FlexDirection::Column, width: 100pct, margin_bottom: 1) { #(elements) })
}

/// Formats the active prompt with default width for compatibility tests.
///
/// This is a flattened compatibility adapter; live prompt rendering should use
/// the `Prompt` component.
pub fn prompt_render_lines(state: &State) -> Vec<RenderLine> {
    prompt_render_lines_with_width(state, None)
}

/// Formats the active prompt with width-aware wrapping.
///
/// This is a flattened compatibility adapter; live prompt rendering should use
/// the `Prompt` component.
pub(crate) fn prompt_render_lines_with_width(state: &State, width: Option<u16>) -> Vec<RenderLine> {
    prompt_rows(state, width)
}

pub(crate) fn prompt_row_count(state: &State, width: Option<u16>) -> usize {
    prompt_rows(state, width).len()
}

fn prompt_rows(state: &State, width: Option<u16>) -> Vec<RenderLine> {
    crate::prompt::render_lines(&state.session.prompt, &state.commands, width)
}

/// Formats the active prompt as plain visible text for compatibility tests.
pub fn prompt_lines(state: &State) -> Vec<String> {
    crate::components::plain_lines(prompt_render_lines(state))
}

/// Props for the prompt component.
#[derive(Props)]
pub struct PromptProps {
    pub state: Arc<State>,
    pub width: Option<u16>,
}
