use super::content::{transcript_item_frame, TranscriptRowWrap};
use super::TranscriptRenderContext;
use crate::render::RenderLine;
use iocraft::prelude::*;

/// Renders a notice transcript item.
#[component]
pub fn Notice(props: &NoticeProps) -> impl Into<AnyElement<'static>> {
    let rows = notice_render_lines(&props.message);

    transcript_item_frame(
        props.source_id.clone(),
        rows,
        props.context.clone(),
        TranscriptRowWrap::Auto,
    )
}

/// Formats notice content as one text row.
pub(super) fn notice_render_lines(message: &str) -> Vec<RenderLine> {
    vec![RenderLine::text(message)]
}

/// Props for the notice component.
#[derive(Props)]
pub struct NoticeProps {
    pub source_id: String,
    pub message: String,
    pub context: TranscriptRenderContext,
}
