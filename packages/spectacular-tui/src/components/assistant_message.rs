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
    let lines = render_lines_elements(styled_visible_lines(&message.text, RenderStyle::Assistant));

    element!(View(flex_direction: FlexDirection::Column) { #(lines.into_iter()) })
}

/// Props for the assistant-message component.
#[derive(Default, Props)]
pub struct AssistantMessageProps {
    pub item: Option<TranscriptItem>,
}
