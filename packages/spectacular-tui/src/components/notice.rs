use crate::components::transcript_content::render_lines_elements;
use crate::render_model::RenderLine;
use crate::transcript::{TranscriptItem, TranscriptItemContent};
use iocraft::prelude::*;

/// Renders a notice transcript item.
#[component]
pub fn Notice(props: &NoticeProps) -> impl Into<AnyElement<'static>> {
    let item = props.item.clone().expect("Notice requires item");
    let TranscriptItemContent::Notice(notice) = item.content else {
        panic!("Notice requires notice content");
    };
    let lines = render_lines_elements(notice_render_lines(&notice.message));

    element!(View(flex_direction: FlexDirection::Column) { #(lines.into_iter()) })
}

/// Formats notice content as one text row.
pub fn notice_render_lines(message: &str) -> Vec<RenderLine> {
    vec![RenderLine::text(message)]
}

/// Props for the notice component.
#[derive(Default, Props)]
pub struct NoticeProps {
    pub item: Option<TranscriptItem>,
}
