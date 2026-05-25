use super::content::{styled_visible_lines, transcript_item_frame, TranscriptRowWrap};
use super::TranscriptRenderContext;
use crate::render::RenderStyle;
use iocraft::prelude::*;

/// Renders an assistant-message transcript item.
#[component]
pub fn Assistant(props: &AssistantProps) -> impl Into<AnyElement<'static>> {
    let rows = assistant_message_render_lines(&props.text);

    transcript_item_frame(
        props.source_id.clone(),
        rows,
        props.context.clone(),
        TranscriptRowWrap::Auto,
    )
}

/// Formats assistant message content as semantic rows.
pub(super) fn assistant_message_render_lines(text: &str) -> Vec<crate::render::RenderLine> {
    styled_visible_lines(text, RenderStyle::Assistant)
}

/// Props for the assistant component.
#[derive(Props)]
pub struct AssistantProps {
    pub source_id: String,
    pub text: String,
    pub context: TranscriptRenderContext,
}
