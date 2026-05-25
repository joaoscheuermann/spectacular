use super::content::{transcript_item_frame, TranscriptRowWrap, TRANSCRIPT_SEPARATOR};
use super::TranscriptRenderContext;
use crate::render::{RenderLine, RenderStyle};
use iocraft::prelude::*;

/// Renders a completed work-summary transcript item.
#[component]
pub fn Summary(props: &SummaryProps) -> impl Into<AnyElement<'static>> {
    let rows = worked_summary_render_lines(&props.duration, props.turn_tokens);

    transcript_item_frame(
        props.source_id.clone(),
        rows,
        props.context.clone(),
        TranscriptRowWrap::Auto,
    )
}

/// Formats a completed work summary with duration and turn-token count.
pub(super) fn worked_summary_render_lines(
    duration: &str,
    turn_tokens: Option<u64>,
) -> Vec<RenderLine> {
    let summary = format!(
        "Worked for {duration}{TRANSCRIPT_SEPARATOR}total {} tokens",
        turn_tokens.unwrap_or(0)
    );
    vec![RenderLine::styled(summary, RenderStyle::Dim)]
}

/// Props for the summary component.
#[derive(Props)]
pub struct SummaryProps {
    pub source_id: String,
    pub duration: String,
    pub turn_tokens: Option<u64>,
    pub context: TranscriptRenderContext,
}
