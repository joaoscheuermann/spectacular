use crate::format_directory::format_directory;
use crate::render_model::{iocraft_content, RenderLine, RenderSpan, RenderStyle};
use crate::transcript::{OpeningBannerItem, TranscriptItem, TranscriptItemContent};
use iocraft::prelude::*;
use std::path::Path;
use unicode_width::UnicodeWidthStr;

const OPENING_BANNER_MIN_WIDTH: usize = 52;

/// Renders an opening-banner transcript item.
#[component]
pub fn OpeningBanner(props: &OpeningBannerProps) -> impl Into<AnyElement<'static>> {
    let item = props.item.clone().expect("OpeningBanner requires item");
    let TranscriptItemContent::OpeningBanner(banner) = item.content else {
        panic!("OpeningBanner requires opening-banner content");
    };
    let elements = opening_banner_render_lines(&banner)
        .into_iter()
        .map(|line| {
            let contents = iocraft_content(&line);
            element!(MixedText(wrap: TextWrap::NoWrap, contents))
        });

    element!(View(flex_direction: FlexDirection::Column, margin_bottom: 1) { #(elements) })
}

/// Formats the opening banner as fixed-width box-drawing rows.
pub fn opening_banner_render_lines(banner: &OpeningBannerItem) -> Vec<RenderLine> {
    let rows = opening_banner_rows(banner);
    let content_width = rows
        .iter()
        .map(|row| UnicodeWidthStr::width(row.text.as_str()))
        .max()
        .unwrap_or(0)
        .max(OPENING_BANNER_MIN_WIDTH);
    let horizontal = "─".repeat(content_width + 2);

    let mut lines = vec![RenderLine::styled(
        format!("╭{horizontal}╮"),
        RenderStyle::Title,
    )];
    lines.extend(
        rows.iter()
            .map(|row| opening_banner_content_line(row, content_width)),
    );
    lines.push(RenderLine::styled(
        format!("╰{horizontal}╯"),
        RenderStyle::Title,
    ));
    lines
}

/// Builds display-ready opening banner rows with semantic content styles.
fn opening_banner_rows(banner: &OpeningBannerItem) -> Vec<OpeningBannerRow> {
    vec![
        OpeningBannerRow::new(
            format!("Spectacular (v{})", banner.version),
            RenderStyle::Title,
        ),
        OpeningBannerRow::new(String::new(), RenderStyle::Text),
        OpeningBannerRow::new(
            format!("model:     {} {}", banner.model, banner.reasoning),
            RenderStyle::Text,
        ),
        OpeningBannerRow::new(
            format!(
                "directory: {}",
                format_directory(Path::new(&banner.directory))
            ),
            RenderStyle::Text,
        ),
        OpeningBannerRow::new(
            format!("session:   {}", banner.session_id),
            RenderStyle::Text,
        ),
    ]
}

/// Formats one opening banner content row with styled borders and inner text.
fn opening_banner_content_line(row: &OpeningBannerRow, width: usize) -> RenderLine {
    let padding = width.saturating_sub(UnicodeWidthStr::width(row.text.as_str()));
    RenderLine::from_spans(vec![
        RenderSpan::new("│ ", RenderStyle::Title),
        RenderSpan::new(&row.text, row.style),
        RenderSpan::new(" ".repeat(padding), row.style),
        RenderSpan::new(" │", RenderStyle::Title),
    ])
}

/// One display row inside the opening banner with its semantic content style.
struct OpeningBannerRow {
    text: String,
    style: RenderStyle,
}

impl OpeningBannerRow {
    /// Creates an opening-banner row from display text and semantic content style.
    fn new(text: impl Into<String>, style: RenderStyle) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }
}

/// Props for the opening-banner component.
#[derive(Default, Props)]
pub struct OpeningBannerProps {
    pub item: Option<TranscriptItem>,
}
