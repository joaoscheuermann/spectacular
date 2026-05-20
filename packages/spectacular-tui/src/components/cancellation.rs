use crate::components::transcript_content::render_lines_elements;
use crate::render_model::{RenderLine, RenderStyle};
use crate::transcript::{TranscriptItem, TranscriptItemContent};
use iocraft::prelude::*;

/// Renders a cancellation transcript item.
#[component]
pub fn Cancellation(props: &CancellationProps) -> impl Into<AnyElement<'static>> {
    let item = props.item.clone().expect("Cancellation requires item");
    let TranscriptItemContent::Cancellation(cancellation) = item.content else {
        panic!("Cancellation requires cancellation content");
    };
    let lines = render_lines_elements(cancellation_render_lines(&cancellation.reason));

    element!(View(flex_direction: FlexDirection::Column) { #(lines.into_iter()) })
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
