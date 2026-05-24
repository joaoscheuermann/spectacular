use super::content::{selectable_line, selectable_text_wrap};
use super::TranscriptRenderContext;
use crate::render::{iocraft_content_with_selection_colors, RenderLine};
use crate::transcript::{TranscriptItem, TranscriptItemContent};
use iocraft::prelude::*;

/// Renders a notice transcript item.
#[component]
pub fn Notice(props: &NoticeProps) -> impl Into<AnyElement<'static>> {
    let item = props.item.clone().expect("Notice requires item");
    let context = props.context.as_ref();
    let selection_colors = context
        .map(|context| context.selection_colors)
        .unwrap_or_default();
    let item_id = item.id.as_str().to_owned();
    let TranscriptItemContent::Notice(notice) = item.content else {
        panic!("Notice requires notice content");
    };
    let elements = notice_render_lines(&notice.message)
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

/// Formats notice content as one text row.
pub fn notice_render_lines(message: &str) -> Vec<RenderLine> {
    vec![RenderLine::text(message)]
}

/// Props for the notice component.
#[derive(Default, Props)]
pub struct NoticeProps {
    pub item: Option<TranscriptItem>,
    pub context: Option<TranscriptRenderContext>,
}
