use super::content::{selectable_line, selectable_text_wrap};
use super::TranscriptRenderContext;
use crate::render::{iocraft_content_with_selection_colors, RenderLine, RenderStyle};
use crate::transcript::{TranscriptItem, TranscriptItemContent};
use iocraft::prelude::*;

/// Renders a cancellation transcript item.
#[component]
pub fn Cancellation(props: &CancellationProps) -> impl Into<AnyElement<'static>> {
    let item = props.item.clone().expect("Cancellation requires item");
    let context = props.context.as_ref();
    let selection_colors = context
        .map(|context| context.selection_colors)
        .unwrap_or_default();
    let item_id = item.id.as_str().to_owned();
    let TranscriptItemContent::Cancellation(cancellation) = item.content else {
        panic!("Cancellation requires cancellation content");
    };
    let elements = cancellation_render_lines(&cancellation.reason)
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

/// Formats cancellation content as one warning row.
pub fn cancellation_render_lines(reason: &str) -> Vec<RenderLine> {
    vec![RenderLine::styled(reason, RenderStyle::Warning)]
}

/// Props for the cancellation component.
#[derive(Default, Props)]
pub struct CancellationProps {
    pub item: Option<TranscriptItem>,
    pub context: Option<TranscriptRenderContext>,
}
