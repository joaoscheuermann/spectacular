use crate::render_model::{iocraft_content, RenderLine, RenderStyle};
use crate::transcript::{TranscriptItem, TranscriptItemContent};
use iocraft::prelude::*;

/// Renders a success transcript item.
#[component]
pub fn Success(props: &SuccessProps) -> impl Into<AnyElement<'static>> {
    let item = props.item.clone().expect("Success requires item");
    let TranscriptItemContent::Success(success) = item.content else {
        panic!("Success requires success content");
    };
    let elements = success_render_lines(&success.message)
        .into_iter()
        .map(|line| {
            let contents = iocraft_content(&line);
            element!(MixedText(wrap: TextWrap::Wrap, contents))
        });

    element!(View(flex_direction: FlexDirection::Column) { #(elements) })
}

/// Formats success content as one semantic row.
pub fn success_render_lines(message: &str) -> Vec<RenderLine> {
    vec![RenderLine::styled(message, RenderStyle::Success)]
}

/// Props for the success component.
#[derive(Default, Props)]
pub struct SuccessProps {
    pub item: Option<TranscriptItem>,
}
