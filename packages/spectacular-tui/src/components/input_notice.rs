use crate::render::{iocraft_content_with_selection_colors, RenderLine, RenderStyle};
use crate::selection::{style_line_for_source, SelectableSource};
use crate::state::State;
use iocraft::prelude::*;
use std::sync::Arc;

/// Renders transient input feedback near the active prompt.
#[component]
pub fn InputNotice(props: &InputNoticeProps) -> impl Into<AnyElement<'static>> {
    let state = props.state.as_ref();
    let Some(line) = input_notice_line(state) else {
        return element!(View(width: 100pct)).into_any();
    };

    let line = style_line_for_source(state, line, SelectableSource::InputNotice);
    let contents = iocraft_content_with_selection_colors(&line, state.selection_colors);
    element!(View(width: 100pct) {
        MixedText(wrap: TextWrap::NoWrap, contents)
    })
    .into_any()
}

/// Formats prompt-local input feedback when present.
///
/// This is a flattened compatibility adapter; live rendering should use the
/// `InputNotice` component.
pub(crate) fn input_notice_render_line(state: &State) -> Option<RenderLine> {
    input_notice_line(state)
}

fn input_notice_line(state: &State) -> Option<RenderLine> {
    state
        .input_notice
        .as_ref()
        .map(|message| RenderLine::styled(format!("  {message}"), RenderStyle::Dim))
}

/// Props for the input notice component.
#[derive(Props)]
pub struct InputNoticeProps {
    pub state: Arc<State>,
}
