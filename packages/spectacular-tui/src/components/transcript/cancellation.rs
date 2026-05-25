use super::content::{transcript_item_frame, TranscriptRowWrap};
use super::TranscriptRenderContext;
use crate::render::{RenderLine, RenderStyle};
use iocraft::prelude::*;

/// Renders a cancellation transcript item.
#[component]
pub fn Cancellation(props: &CancellationProps) -> impl Into<AnyElement<'static>> {
    let rows = cancellation_render_lines(&props.reason);

    transcript_item_frame(
        props.source_id.clone(),
        rows,
        props.context.clone(),
        TranscriptRowWrap::Auto,
    )
}

/// Formats cancellation content as one warning row.
pub(super) fn cancellation_render_lines(reason: &str) -> Vec<RenderLine> {
    vec![RenderLine::styled(reason, RenderStyle::Warning)]
}

/// Props for the cancellation component.
#[derive(Props)]
pub struct CancellationProps {
    pub source_id: String,
    pub reason: String,
    pub context: TranscriptRenderContext,
}
