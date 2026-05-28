use super::content::{transcript_item_frame, TranscriptRowWrap};
use super::TranscriptRenderContext;
use crate::render::format_directory;
use crate::render::{RenderLine, RenderSpan, RenderStyle};
use crate::transcript::OpeningBannerItem;
use iocraft::prelude::*;
use std::path::Path;
use unicode_width::UnicodeWidthStr;

const OPENING_BANNER_MIN_WIDTH: usize = 52;

/// Renders an opening-banner transcript item.
#[component]
pub fn Banner(props: &BannerProps) -> impl Into<AnyElement<'static>> {
    let rows = opening_banner_render_lines_from_fields(
        &props.version,
        &props.model,
        &props.reasoning,
        &props.directory,
        &props.session_id,
    );

    transcript_item_frame(
        props.source_id.clone(),
        rows,
        props.context.clone(),
        TranscriptRowWrap::NoWrap,
    )
}

/// Formats the opening banner as fixed-width box-drawing rows.
pub(super) fn opening_banner_render_lines(banner: &OpeningBannerItem) -> Vec<RenderLine> {
    opening_banner_render_lines_from_fields(
        &banner.version,
        &banner.model,
        &banner.reasoning,
        &banner.directory,
        &banner.session_id,
    )
}

fn opening_banner_render_lines_from_fields(
    version: &str,
    model: &str,
    reasoning: &str,
    directory: &str,
    session_id: &str,
) -> Vec<RenderLine> {
    let rows = opening_banner_rows(version, model, reasoning, directory, session_id);
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
fn opening_banner_rows(
    version: &str,
    model: &str,
    reasoning: &str,
    directory: &str,
    session_id: &str,
) -> Vec<OpeningBannerRow> {
    vec![
        OpeningBannerRow::new(format!("Doric (v{version})"), RenderStyle::Title),
        OpeningBannerRow::new(String::new(), RenderStyle::Text),
        OpeningBannerRow::new(format!("model:     {model} {reasoning}"), RenderStyle::Text),
        OpeningBannerRow::new(
            format!("directory: {}", format_directory(Path::new(directory))),
            RenderStyle::Text,
        ),
        OpeningBannerRow::new(format!("session:   {session_id}"), RenderStyle::Text),
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
#[derive(Props)]
pub struct BannerProps {
    pub source_id: String,
    pub version: String,
    pub model: String,
    pub reasoning: String,
    pub directory: String,
    pub session_id: String,
    pub context: TranscriptRenderContext,
}
