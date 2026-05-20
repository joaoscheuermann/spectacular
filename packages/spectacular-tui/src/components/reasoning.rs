use crate::components::transcript_content::styled_visible_trimmed_lines;
use crate::render_model::{iocraft_content, RenderStyle};
use crate::transcript::{TranscriptItem, TranscriptItemContent};
use iocraft::prelude::*;

/// Renders a reasoning transcript item.
#[component]
pub fn Reasoning(props: &ReasoningProps) -> impl Into<AnyElement<'static>> {
    let item = props.item.clone().expect("Reasoning requires item");
    let TranscriptItemContent::Reasoning(reasoning) = item.content else {
        panic!("Reasoning requires reasoning content");
    };
    let elements = reasoning_render_lines(&reasoning.text)
        .into_iter()
        .map(|line| {
            let contents = iocraft_content(&line);
            element!(MixedText(wrap: TextWrap::Wrap, contents))
        });

    element!(View(flex_direction: FlexDirection::Column) { #(elements) })
}

/// Formats reasoning content as non-blank semantic rows.
pub fn reasoning_render_lines(text: &str) -> Vec<crate::render_model::RenderLine> {
    styled_visible_trimmed_lines(text, RenderStyle::Reasoning)
}

/// Props for the reasoning component.
#[derive(Default, Props)]
pub struct ReasoningProps {
    pub item: Option<TranscriptItem>,
}
