use super::content::{transcript_item_frame, TranscriptRowWrap};
use super::TranscriptRenderContext;
use crate::render::{RenderLine, RenderStyle};
use iocraft::prelude::*;

/// Renders a success transcript item.
#[component]
pub fn Success(props: &SuccessProps) -> impl Into<AnyElement<'static>> {
    let rows = success_render_lines(&props.message);

    transcript_item_frame(
        props.source_id.clone(),
        rows,
        props.context.clone(),
        TranscriptRowWrap::Auto,
    )
}

/// Formats success content as one semantic row.
pub(super) fn success_render_lines(message: &str) -> Vec<RenderLine> {
    vec![RenderLine::styled(message, RenderStyle::Success)]
}

/// Props for the success component.
#[derive(Props)]
pub struct SuccessProps {
    pub source_id: String,
    pub message: String,
    pub context: TranscriptRenderContext,
}
