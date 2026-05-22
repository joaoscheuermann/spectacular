use crate::render::{RenderLine, RenderStyle};
use crate::transcript::DisplayLine;

/// Separator used by completed work summaries.
pub const TRANSCRIPT_SEPARATOR: &str = " · ";

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
