use super::content::{selectable_line, selectable_text_wrap};
use super::TranscriptRenderContext;
use crate::render::{iocraft_content_with_selection_colors, RenderLine, RenderStyle};
use crate::transcript::{TranscriptItem, TranscriptItemContent};
use iocraft::prelude::*;

/// Renders a warning transcript item.
#[component]
pub fn Warning(props: &WarningProps) -> impl Into<AnyElement<'static>> {
    let item = props.item.clone().expect("Warning requires item");
    let context = props.context.as_ref();
    let selection_colors = context
        .map(|context| context.selection_colors)
        .unwrap_or_default();
    let item_id = item.id.as_str().to_owned();
    let TranscriptItemContent::Warning(warning) = item.content else {
        panic!("Warning requires warning content");
    };
    let elements = warning_render_lines(&warning.message)
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

/// Formats warning content as one semantic row.
pub fn warning_render_lines(message: &str) -> Vec<RenderLine> {
    vec![RenderLine::styled(
        format!("warning: {message}"),
        RenderStyle::Warning,
    )]
}

/// Props for the warning component.
#[derive(Default, Props)]
pub struct WarningProps {
    pub item: Option<TranscriptItem>,
    pub context: Option<TranscriptRenderContext>,
}
