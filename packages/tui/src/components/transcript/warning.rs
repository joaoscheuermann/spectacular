use super::content::{transcript_item_frame, TranscriptRowWrap};
use super::TranscriptRenderContext;
use crate::render::{RenderLine, RenderStyle};
use iocraft::prelude::*;

/// Renders a warning transcript item.
#[component]
pub fn Warning(props: &WarningProps) -> impl Into<AnyElement<'static>> {
    let rows = warning_render_lines(&props.message);

    transcript_item_frame(
        props.source_id.clone(),
        rows,
        props.context.clone(),
        TranscriptRowWrap::Auto,
    )
}

/// Formats warning content as one semantic row.
pub(super) fn warning_render_lines(message: &str) -> Vec<RenderLine> {
    vec![RenderLine::styled(
        format!("warning: {message}"),
        RenderStyle::Warning,
    )]
}

/// Props for the warning component.
#[derive(Props)]
pub struct WarningProps {
    pub source_id: String,
    pub message: String,
    pub context: TranscriptRenderContext,
}
