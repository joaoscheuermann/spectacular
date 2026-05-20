use crate::components::transcript_content::styled_visible_lines;
use crate::render_model::{iocraft_content, RenderStyle};
use crate::transcript::{TranscriptItem, TranscriptItemContent};
use iocraft::prelude::*;

/// Renders an assistant-message transcript item.
#[component]
pub fn AssistantMessage(props: &AssistantMessageProps) -> impl Into<AnyElement<'static>> {
    let item = props.item.clone().expect("AssistantMessage requires item");

    let TranscriptItemContent::AssistantMessage(message) = item.content else {
        panic!("AssistantMessage requires assistant-message content");
    };

    let lines = assistant_message_render_lines(&message.text);
    let elements = lines.into_iter().map(|line| {
        let contents = iocraft_content(&line);
        element!(MixedText(wrap: TextWrap::Wrap, contents))
    });

    element!(View(flex_direction: FlexDirection::Column) { #(elements) })
}

/// Formats assistant message content as semantic rows.
pub fn assistant_message_render_lines(text: &str) -> Vec<crate::render_model::RenderLine> {
    styled_visible_lines(text, RenderStyle::Assistant)
}

/// Props for the assistant-message component.
#[derive(Default, Props)]
pub struct AssistantMessageProps {
    pub item: Option<TranscriptItem>,
}
