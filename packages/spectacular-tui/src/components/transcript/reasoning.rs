use super::content::{selectable_line, selectable_text_wrap, styled_visible_trimmed_lines};
use super::TranscriptRenderContext;
use crate::render::{iocraft_content_with_selection_colors, RenderStyle};
use crate::transcript::{TranscriptItem, TranscriptItemContent};
use iocraft::prelude::*;

/// Renders a reasoning transcript item.
#[component]
pub fn Reasoning(props: &ReasoningProps) -> impl Into<AnyElement<'static>> {
    let item = props.item.clone().expect("Reasoning requires item");
    let context = props.context.as_ref();
    let selection_colors = context
        .map(|context| context.selection_colors)
        .unwrap_or_default();
    let item_id = item.id.as_str().to_owned();
    let TranscriptItemContent::Reasoning(reasoning) = item.content else {
        panic!("Reasoning requires reasoning content");
    };
    let elements = reasoning_render_lines(&reasoning.text)
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

/// Formats reasoning content as non-blank semantic rows.
pub fn reasoning_render_lines(text: &str) -> Vec<crate::render::RenderLine> {
    styled_visible_trimmed_lines(text, RenderStyle::Reasoning)
}

/// Props for the reasoning component.
#[derive(Default, Props)]
pub struct ReasoningProps {
    pub item: Option<TranscriptItem>,
    pub context: Option<TranscriptRenderContext>,
}
