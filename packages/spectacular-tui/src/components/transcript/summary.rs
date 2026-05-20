use super::content::TRANSCRIPT_SEPARATOR;
use crate::render::{iocraft_content, RenderLine, RenderStyle};
use crate::transcript::{TranscriptItem, TranscriptItemContent};
use iocraft::prelude::*;

/// Renders a completed work-summary transcript item.
#[component]
pub fn Summary(props: &SummaryProps) -> impl Into<AnyElement<'static>> {
    let item = props.item.clone().expect("Summary requires item");
    let TranscriptItemContent::WorkedSummary(worked_summary) = item.content else {
        panic!("Summary requires worked-summary content");
    };
    let elements =
        worked_summary_render_lines(&worked_summary.duration, worked_summary.turn_tokens)
            .into_iter()
            .map(|line| {
                let contents = iocraft_content(&line);
                element!(MixedText(wrap: TextWrap::Wrap, contents))
            });

    element!(View(flex_direction: FlexDirection::Column, margin_bottom: 1) { #(elements) })
}

/// Formats a completed work summary with duration and turn-token count.
pub fn worked_summary_render_lines(duration: &str, turn_tokens: Option<u64>) -> Vec<RenderLine> {
    let summary = format!(
        "Worked for {duration}{TRANSCRIPT_SEPARATOR}total {} tokens",
        turn_tokens.unwrap_or(0)
    );
    vec![RenderLine::styled(summary, RenderStyle::Dim)]
}

/// Props for the summary component.
#[derive(Default, Props)]
pub struct SummaryProps {
    pub item: Option<TranscriptItem>,
}
