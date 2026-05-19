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
    let lines = render_lines_elements(vec![RenderLine::styled(
        format!("warning: {}", warning.message),
        RenderStyle::Warning,
    )]);

    element!(View(flex_direction: FlexDirection::Column) { #(lines.into_iter()) })
}

/// Props for the warning component.
#[derive(Default, Props)]
pub struct WarningProps {
    pub item: Option<TranscriptItem>,
}
