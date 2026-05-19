use crate::components::transcript_content::{render_lines_elements, submitted_prompt_render_lines};
use crate::render_model::RenderStyle;
use crate::transcript::{TranscriptItem, TranscriptItemContent};
use iocraft::prelude::*;

/// Renders a submitted user-prompt transcript item.
#[component]
pub fn UserPrompt(props: &UserPromptProps) -> impl Into<AnyElement<'static>> {
    let item = props.item.clone().expect("UserPrompt requires item");
    let TranscriptItemContent::UserPrompt(prompt) = item.content else {
        panic!("UserPrompt requires user-prompt content");
    };
    let lines = render_lines_elements(submitted_prompt_render_lines(
        &prompt.text,
        RenderStyle::User,
    ));

    element!(View(flex_direction: FlexDirection::Column) { #(lines.into_iter()) })
}

/// Props for the user-prompt component.
#[derive(Default, Props)]
pub struct UserPromptProps {
    pub item: Option<TranscriptItem>,
}
