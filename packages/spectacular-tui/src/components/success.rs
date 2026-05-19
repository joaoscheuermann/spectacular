use crate::components::transcript_content::render_lines_elements;
use crate::render_model::{RenderLine, RenderStyle};
use crate::transcript::{TranscriptItem, TranscriptItemContent};
use iocraft::prelude::*;

/// Renders a success transcript item.
#[component]
pub fn Success(props: &SuccessProps) -> impl Into<AnyElement<'static>> {
    let item = props.item.clone().expect("Success requires item");
    let TranscriptItemContent::Success(success) = item.content else {
        panic!("Success requires success content");
    };
    let lines = render_lines_elements(vec![RenderLine::styled(
        &success.message,
        RenderStyle::Success,
    )]);

    element!(View(flex_direction: FlexDirection::Column) { #(lines.into_iter()) })
}

/// Props for the success component.
#[derive(Default, Props)]
pub struct SuccessProps {
    pub item: Option<TranscriptItem>,
}
