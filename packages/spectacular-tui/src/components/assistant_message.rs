use crate::components::transcript_content::{render_lines_elements, styled_visible_lines};
use crate::render_model::RenderStyle;
use crate::transcript::{TranscriptItem, TranscriptItemContent};
use iocraft::prelude::*;

/// Renders an assistant-message transcript item.
#[component]
pub fn AssistantMessage(props: &AssistantMessageProps) -> impl Into<AnyElement<'static>> {
    let item = props.item.clone().expect("AssistantMessage requires item");

    let TranscriptItemContent::AssistantMessage(message) = item.content else {
        panic!("AssistantMessage requires assistant-message content");
    };

    let lines = render_lines_elements(assistant_message_render_lines(&message.text));

    element!(View(flex_direction: FlexDirection::Column) { #(lines.into_iter()) })
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
