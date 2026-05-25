use super::content::{styled_visible_lines, transcript_item_frame, TranscriptRowWrap};
use super::TranscriptRenderContext;
use crate::render::RenderStyle;
use iocraft::prelude::*;

/// Renders a submitted user-prompt transcript item.
#[component]
pub fn User(props: &UserProps) -> impl Into<AnyElement<'static>> {
    let rows = user_prompt_render_lines(&props.text);

    transcript_item_frame(
        props.source_id.clone(),
        rows,
        props.context.clone(),
        TranscriptRowWrap::Auto,
    )
}

/// Formats submitted user prompt content as unmarked user rows.
pub(super) fn user_prompt_render_lines(text: &str) -> Vec<crate::render::RenderLine> {
    styled_visible_lines(text, RenderStyle::User)
}

/// Props for the user component.
#[derive(Props)]
pub struct UserProps {
    pub source_id: String,
    pub text: String,
    pub context: TranscriptRenderContext,
}
