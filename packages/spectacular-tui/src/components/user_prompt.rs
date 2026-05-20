use crate::components::transcript_content::submitted_prompt_render_lines;
use crate::render_model::{iocraft_content, RenderStyle};
use crate::transcript::{TranscriptItem, TranscriptItemContent};
use iocraft::prelude::*;

/// Renders a submitted user-prompt transcript item.
#[component]
pub fn UserPrompt(props: &UserPromptProps) -> impl Into<AnyElement<'static>> {
    let item = props.item.clone().expect("UserPrompt requires item");
    let TranscriptItemContent::UserPrompt(prompt) = item.content else {
        panic!("UserPrompt requires user-prompt content");
    };
    let elements = user_prompt_render_lines(&prompt.text)
        .into_iter()
        .map(|line| {
            let contents = iocraft_content(&line);
            element!(MixedText(wrap: TextWrap::Wrap, contents))
        });

    element!(View(flex_direction: FlexDirection::Column) { #(elements) })
}

/// Formats submitted user prompt content as prompt-marked rows.
pub fn user_prompt_render_lines(text: &str) -> Vec<crate::render_model::RenderLine> {
    submitted_prompt_render_lines(text, RenderStyle::User)
}

/// Props for the user-prompt component.
#[derive(Default, Props)]
pub struct UserPromptProps {
    pub item: Option<TranscriptItem>,
}
