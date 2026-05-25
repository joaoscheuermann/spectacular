use super::content::{styled_visible_lines, transcript_item_frame, TranscriptRowWrap};
use super::TranscriptRenderContext;
use crate::render::{RenderLine, RenderStyle};
use iocraft::prelude::*;

/// Renders an error transcript item.
#[component]
pub fn Error(props: &ErrorProps) -> impl Into<AnyElement<'static>> {
    let rows = error_render_lines(&props.message, props.details.as_deref());

    transcript_item_frame(
        props.source_id.clone(),
        rows,
        props.context.clone(),
        TranscriptRowWrap::Auto,
    )
}

/// Formats an error transcript item as semantic rows.
pub(super) fn error_render_lines(message: &str, details: Option<&str>) -> Vec<RenderLine> {
    let mut lines = vec![RenderLine::styled(
        format!("error: {message}"),
        RenderStyle::Error,
    )];
    if let Some(details) = details {
        lines.extend(styled_visible_lines(details, RenderStyle::CommandOutput));
    }
    lines
}

/// Props for the error component.
#[derive(Props)]
pub struct ErrorProps {
    pub source_id: String,
    pub message: String,
    pub details: Option<String>,
    pub context: TranscriptRenderContext,
}
