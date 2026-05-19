use crate::components::transcript_content::render_lines_elements;
use crate::format::opening_banner_render_lines;
use crate::transcript::{TranscriptItem, TranscriptItemContent};
use iocraft::prelude::*;

/// Renders an opening-banner transcript item using semantic render formatting.
#[component]
pub fn OpeningBanner(props: &OpeningBannerProps) -> impl Into<AnyElement<'static>> {
    let item = props.item.clone().expect("OpeningBanner requires item");
    let TranscriptItemContent::OpeningBanner(banner) = item.content else {
        panic!("OpeningBanner requires opening-banner content");
    };
    let lines = render_lines_elements(opening_banner_render_lines(&banner));

    element!(View(flex_direction: FlexDirection::Column) { #(lines.into_iter()) })
}

/// Props for the opening-banner component.
#[derive(Default, Props)]
pub struct OpeningBannerProps {
    pub item: Option<TranscriptItem>,
}
