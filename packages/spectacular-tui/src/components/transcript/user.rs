use super::content::styled_visible_lines;
use crate::render::{iocraft_content, RenderStyle};
use crate::transcript::{TranscriptItem, TranscriptItemContent};
use iocraft::prelude::*;

/// Renders a submitted user-prompt transcript item.
#[component]
pub fn User(props: &UserProps) -> impl Into<AnyElement<'static>> {
    let item = props.item.clone().expect("User requires item");
    let TranscriptItemContent::UserPrompt(prompt) = item.content else {
        panic!("User requires user-prompt content");
    };
    let elements = user_prompt_render_lines(&prompt.text)
        .into_iter()
        .map(|line| {
            let contents = iocraft_content(&line);
            element!(MixedText(wrap: TextWrap::Wrap, contents))
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
}
