use super::content::{selectable_line, selectable_text_wrap, styled_visible_lines};
use super::TranscriptRenderContext;
use crate::render::{iocraft_content_with_selection_colors, RenderStyle};
use crate::transcript::{TranscriptItem, TranscriptItemContent};
use iocraft::prelude::*;

/// Renders a submitted user-prompt transcript item.
#[component]
pub fn User(props: &UserProps) -> impl Into<AnyElement<'static>> {
    let item = props.item.clone().expect("User requires item");
    let context = props.context.as_ref();
    let selection_colors = context
        .map(|context| context.selection_colors)
        .unwrap_or_default();
    let item_id = item.id.as_str().to_owned();
    let TranscriptItemContent::UserPrompt(prompt) = item.content else {
        panic!("User requires user-prompt content");
    };
    let elements = user_prompt_render_lines(&prompt.text)
        .into_iter()
        .enumerate()
        .map(|(index, line)| {
            let line = selectable_line(context, &item_id, index, line);
            let wrap = selectable_text_wrap(context, &line);
            let contents = iocraft_content_with_selection_colors(&line, selection_colors);
            element!(MixedText(wrap: wrap, contents))
        });

    element!(View(flex_direction: FlexDirection::Column, margin_bottom: 1) { #(elements) })
}

/// Formats submitted user prompt content as unmarked user rows.
pub fn user_prompt_render_lines(text: &str) -> Vec<crate::render::RenderLine> {
    styled_visible_lines(text, RenderStyle::User)
}

/// Props for the user component.
#[derive(Default, Props)]
pub struct UserProps {
    pub item: Option<TranscriptItem>,
    pub context: Option<TranscriptRenderContext>,
}
