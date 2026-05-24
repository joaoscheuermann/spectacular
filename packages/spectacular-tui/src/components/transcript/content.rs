use super::TranscriptRenderContext;
use crate::render::{RenderHighlight, RenderLine, RenderStyle};
use crate::selection::{style_line_for_source_with_plan, SelectableSource};
use crate::transcript::DisplayLine;
use iocraft::prelude::TextWrap;
use unicode_width::UnicodeWidthStr;

/// Separator used by completed work summaries.
pub const TRANSCRIPT_SEPARATOR: &str = " \u{00b7} ";

/// Flattens semantic rows into plain visible text rows.
pub fn plain_lines(lines: Vec<RenderLine>) -> Vec<String> {
    lines.into_iter().map(|line| line.plain_text()).collect()
}

/// Converts one adapter display line into one semantic render row.
pub fn display_line_render_line(line: &DisplayLine) -> RenderLine {
    if line.spans.is_empty() {
        return RenderLine::styled(&line.text, RenderStyle::from(line.style));
    }

    RenderLine::from_spans(
        line.spans
            .iter()
            .map(|span| crate::render::RenderSpan::new(&span.text, RenderStyle::from(span.style)))
            .collect(),
    )
}

/// Applies app-wide rendered selection styling to a transcript-owned semantic row.
pub fn selectable_line(
    context: Option<&TranscriptRenderContext>,
    item_id: &str,
    line_index: usize,
    line: RenderLine,
) -> RenderLine {
    let Some(context) = context else {
        return line;
    };

    style_line_for_source_with_plan(
        &context.selection_plan,
        line,
        SelectableSource::Transcript {
            item_id: item_id.to_owned(),
            line: line_index,
        },
        0,
        true,
    )
}

/// Chooses wrapping for a transcript row after rendered-selection styling is applied.
pub fn selectable_text_wrap(
    context: Option<&TranscriptRenderContext>,
    line: &RenderLine,
) -> TextWrap {
    if has_trailing_selected_spaces(line) && line_width(line) <= transcript_content_width(context) {
        return TextWrap::NoWrap;
    }

    TextWrap::Wrap
}

/// Formats visible lines with one semantic style per row.
pub fn styled_visible_lines(text: &str, style: RenderStyle) -> Vec<RenderLine> {
    visible_lines(text)
        .into_iter()
        .map(|line| RenderLine::styled(line, style))
        .collect()
}

/// Formats non-blank visible lines with one semantic style per row.
pub fn styled_visible_trimmed_lines(text: &str, style: RenderStyle) -> Vec<RenderLine> {
    visible_trimmed_lines(text)
        .into_iter()
        .map(|line| RenderLine::styled(line, style))
        .collect()
}

fn has_trailing_selected_spaces(line: &RenderLine) -> bool {
    line.spans.last().is_some_and(|span| {
        span.highlight == Some(RenderHighlight::Selection)
            && span.text.chars().all(|character| character == ' ')
    })
}

fn transcript_content_width(context: Option<&TranscriptRenderContext>) -> usize {
    context
        .map(|context| context.content_width)
        .unwrap_or(usize::MAX)
        .max(1)
}

fn line_width(line: &RenderLine) -> usize {
    UnicodeWidthStr::width(line.plain_text().as_str())
}

/// Splits non-empty text into visible rows without trimming row content.
fn visible_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }

    text.lines().map(ToOwned::to_owned).collect()
}

/// Counts visible rows without allocating row strings.
pub fn visible_text_row_count(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }

    text.lines().count()
}

/// Splits text into visible rows only when it contains non-whitespace content.
fn visible_trimmed_lines(text: &str) -> Vec<String> {
    if text.trim().is_empty() {
        return Vec::new();
    }

    visible_lines(text)
}

/// Counts trimmed visible rows without allocating row strings.
pub fn trimmed_visible_text_row_count(text: &str) -> usize {
    if text.trim().is_empty() {
        return 0;
    }

    visible_text_row_count(text)
}
