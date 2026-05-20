use crate::render_model::{iocraft_content, RenderLine, RenderStyle};
use crate::transcript::{TranscriptItem, TranscriptItemContent};
use iocraft::prelude::*;

/// Renders a cancellation transcript item.
#[component]
pub fn Cancellation(props: &CancellationProps) -> impl Into<AnyElement<'static>> {
    let item = props.item.clone().expect("Cancellation requires item");
    let TranscriptItemContent::Cancellation(cancellation) = item.content else {
        panic!("Cancellation requires cancellation content");
    };
    let elements = cancellation_render_lines(&cancellation.reason)
        .into_iter()
        .map(|line| {
            let contents = iocraft_content(&line);
            element!(MixedText(wrap: TextWrap::Wrap, contents))
        });

    element!(View(flex_direction: FlexDirection::Column, margin_bottom: 1) { #(elements) })
}

/// Formats cancellation content as one warning row.
pub fn cancellation_render_lines(reason: &str) -> Vec<RenderLine> {
    vec![RenderLine::styled(reason, RenderStyle::Warning)]
}

/// Props for the cancellation component.
#[derive(Default, Props)]
pub struct CancellationProps {
    pub item: Option<TranscriptItem>,
}
