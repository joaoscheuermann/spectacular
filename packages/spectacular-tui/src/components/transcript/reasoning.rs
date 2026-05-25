use super::content::{styled_visible_trimmed_lines, transcript_item_frame, TranscriptRowWrap};
use super::TranscriptRenderContext;
use crate::render::RenderStyle;
use iocraft::prelude::*;

/// Renders a reasoning transcript item.
#[component]
pub fn Reasoning(props: &ReasoningProps) -> impl Into<AnyElement<'static>> {
    let rows = reasoning_render_lines(&props.text);

    transcript_item_frame(
        props.source_id.clone(),
        rows,
        props.context.clone(),
        TranscriptRowWrap::Auto,
    )
}

/// Formats reasoning content as non-blank semantic rows.
pub(super) fn reasoning_render_lines(text: &str) -> Vec<crate::render::RenderLine> {
    styled_visible_trimmed_lines(text, RenderStyle::Reasoning)
}

/// Props for the reasoning component.
#[derive(Props)]
pub struct ReasoningProps {
    pub source_id: String,
    pub text: String,
    pub context: TranscriptRenderContext,
}
