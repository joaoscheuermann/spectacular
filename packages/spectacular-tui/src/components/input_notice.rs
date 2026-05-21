use crate::render::{iocraft_content, RenderLine, RenderStyle};
use crate::state::State;
use iocraft::prelude::*;

/// Renders transient input feedback near the active prompt.
#[component]
pub fn InputNotice(props: &InputNoticeProps) -> impl Into<AnyElement<'static>> {
    let state = props.state.clone().expect("InputNotice requires state");
    let Some(line) = input_notice_render_line(&state) else {
        return element!(View(width: 100pct)).into_any();
    };

    let contents = iocraft_content(&line);
    element!(View(width: 100pct) {
        MixedText(wrap: TextWrap::NoWrap, contents)
    })
    .into_any()
}

/// Formats prompt-local input feedback when present.
pub fn input_notice_render_line(state: &State) -> Option<RenderLine> {
    state
        .input_notice
        .as_ref()
        .map(|message| RenderLine::styled(format!("  {message}"), RenderStyle::Dim))
}

/// Props for the input notice component.
#[derive(Default, Props)]
pub struct InputNoticeProps {
    pub state: Option<State>,
}
