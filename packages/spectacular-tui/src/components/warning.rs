use crate::components::transcript_content::render_lines_elements;
use crate::render_model::{RenderLine, RenderStyle};
use crate::transcript::{TranscriptItem, TranscriptItemContent};
use iocraft::prelude::*;

/// Renders a warning transcript item.
#[component]
pub fn Warning(props: &WarningProps) -> impl Into<AnyElement<'static>> {
    let item = props.item.clone().expect("Warning requires item");
    let TranscriptItemContent::Warning(warning) = item.content else {
        panic!("Warning requires warning content");
    };
    let lines = render_lines_elements(warning_render_lines(&warning.message));

    element!(View(flex_direction: FlexDirection::Column) { #(lines.into_iter()) })
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
}
