use super::content::{selectable_line, selectable_text_wrap};
use super::TranscriptRenderContext;
use crate::render::{iocraft_content_with_selection_colors, RenderLine, RenderStyle};
use crate::transcript::{TranscriptItem, TranscriptItemContent};
use iocraft::prelude::*;

/// Renders a success transcript item.
#[component]
pub fn Success(props: &SuccessProps) -> impl Into<AnyElement<'static>> {
    let item = props.item.clone().expect("Success requires item");
    let context = props.context.as_ref();
    let selection_colors = context
        .map(|context| context.selection_colors)
        .unwrap_or_default();
    let item_id = item.id.as_str().to_owned();
    let TranscriptItemContent::Success(success) = item.content else {
        panic!("Success requires success content");
    };
    let elements = success_render_lines(&success.message)
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

/// Formats success content as one semantic row.
pub fn success_render_lines(message: &str) -> Vec<RenderLine> {
    vec![RenderLine::styled(message, RenderStyle::Success)]
}

/// Props for the success component.
#[derive(Default, Props)]
pub struct SuccessProps {
    pub item: Option<TranscriptItem>,
    pub context: Option<TranscriptRenderContext>,
}
