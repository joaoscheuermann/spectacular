use crate::components::transcript_content::{render_lines_elements, styled_visible_trimmed_lines};
use crate::render_model::RenderStyle;
use crate::transcript::{TranscriptItem, TranscriptItemContent};
use iocraft::prelude::*;

/// Renders a reasoning transcript item.
#[component]
pub fn Reasoning(props: &ReasoningProps) -> impl Into<AnyElement<'static>> {
    let item = props.item.clone().expect("Reasoning requires item");
    let TranscriptItemContent::Reasoning(reasoning) = item.content else {
        panic!("Reasoning requires reasoning content");
    };
    let lines = render_lines_elements(reasoning_render_lines(&reasoning.text));

    element!(View(flex_direction: FlexDirection::Column) { #(lines.into_iter()) })
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
