use crate::components::transcript_content::render_lines_elements;
use crate::format::format_directory;
use crate::render_model::{RenderLine, RenderStyle};
use crate::transcript::{OpeningBannerItem, TranscriptItem, TranscriptItemContent};
use iocraft::prelude::*;
use std::path::Path;
use unicode_width::UnicodeWidthStr;

const OPENING_BANNER_MIN_WIDTH: usize = 52;

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

/// Formats the opening banner as original-width Unicode box drawing rows.
fn opening_banner_render_lines(banner: &OpeningBannerItem) -> Vec<RenderLine> {
    let title = format!("Spectacular (v{})", banner.version);
    let spacer = String::new();
    let model = format!("model:     {} {}", banner.model, banner.reasoning);
    let directory = format!(
        "directory: {}",
        format_directory(Path::new(&banner.directory))
    );
    let session = format!("session:   {}", banner.session_id);
    let rows = [&title, &spacer, &model, &directory, &session];
    let content_width = rows
        .iter()
        .map(|line| UnicodeWidthStr::width(line.as_str()))
        .max()
        .unwrap_or(0)
        .max(OPENING_BANNER_MIN_WIDTH);
    let horizontal = "─".repeat(content_width + 2);

    let mut lines = vec![RenderLine::styled(
        format!("╭{horizontal}╮"),
        RenderStyle::Title,
    )];
    lines.extend(rows.iter().map(|line| {
        RenderLine::styled(
            format!("│ {} │", pad_banner_line(line, content_width)),
            RenderStyle::Title,
        )
    }));
    lines.push(RenderLine::styled(
        format!("╰{horizontal}╯"),
        RenderStyle::Title,
    ));
    lines
}

/// Pads a banner row to the computed display width, accounting for Unicode width.
fn pad_banner_line(line: &str, width: usize) -> String {
    let padding = width.saturating_sub(UnicodeWidthStr::width(line));
    format!("{line}{}", " ".repeat(padding))
}
