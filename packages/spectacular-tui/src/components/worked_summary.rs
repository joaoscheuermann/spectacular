use crate::components::transcript_content::{render_lines_elements, TRANSCRIPT_SEPARATOR};
use crate::render_model::{RenderLine, RenderStyle};
use crate::transcript::{TranscriptItem, TranscriptItemContent};
use iocraft::prelude::*;

/// Renders a completed work-summary transcript item.
#[component]
pub fn WorkedSummary(props: &WorkedSummaryProps) -> impl Into<AnyElement<'static>> {
    let item = props.item.clone().expect("WorkedSummary requires item");
    let TranscriptItemContent::WorkedSummary(worked_summary) = item.content else {
        panic!("WorkedSummary requires worked-summary content");
    };
    let summary = format!(
        "Worked for {}{TRANSCRIPT_SEPARATOR}total {} tokens",
        worked_summary.duration,
        worked_summary.turn_tokens.unwrap_or(0)
    );
    let lines = render_lines_elements(vec![RenderLine::styled(summary, RenderStyle::Dim)]);

    element!(View(flex_direction: FlexDirection::Column) { #(lines.into_iter()) })
}

/// Props for the worked-summary component.
#[derive(Default, Props)]
pub struct WorkedSummaryProps {
    pub item: Option<TranscriptItem>,
}
