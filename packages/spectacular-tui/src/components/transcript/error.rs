use super::content::{
    selectable_line, selectable_text_wrap, styled_visible_lines, visible_text_row_count,
};
use super::TranscriptRenderContext;
use crate::render::{iocraft_content_with_selection_colors, RenderLine, RenderStyle};
use crate::transcript::{TranscriptItem, TranscriptItemContent};
use iocraft::prelude::*;

/// Renders an error transcript item.
#[component]
pub fn Error(props: &ErrorProps) -> impl Into<AnyElement<'static>> {
    let item = props.item.clone().expect("Error requires item");
    let context = props.context.as_ref();
    let selection_colors = context
        .map(|context| context.selection_colors)
        .unwrap_or_default();
    let item_id = item.id.as_str().to_owned();
    let TranscriptItemContent::Error(error) = item.content else {
        panic!("Error requires error content");
    };
    let elements = error_render_lines(&error.message, error.details.as_deref())
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

/// Formats an error transcript item as semantic rows.
pub fn error_render_lines(message: &str, details: Option<&str>) -> Vec<RenderLine> {
    let mut lines = vec![RenderLine::styled(
        format!("error: {message}"),
        RenderStyle::Error,
    )];
    if let Some(details) = details {
        lines.extend(styled_visible_lines(details, RenderStyle::CommandOutput));
    }
    lines
}

/// Counts rows for an error item without allocating detail rows.
pub fn error_row_count(details: Option<&str>) -> usize {
    1 + details.map(visible_text_row_count).unwrap_or(0)
}

/// Props for the error component.
#[derive(Default, Props)]
pub struct ErrorProps {
    pub item: Option<TranscriptItem>,
    pub context: Option<TranscriptRenderContext>,
}
