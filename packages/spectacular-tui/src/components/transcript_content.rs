use crate::render_model::{iocraft_content, RenderLine, RenderStyle};
use crate::transcript::DisplayLine;
use iocraft::prelude::*;

/// Separator used by completed work summaries.
pub const TRANSCRIPT_SEPARATOR: &str = " · ";

/// Converts one semantic line into an IOCraft mixed-text element.
pub fn render_line_element(line: RenderLine) -> AnyElement<'static> {
    let contents = iocraft_content(&line);
    element!(MixedText(wrap: TextWrap::NoWrap, contents)).into()
}

/// Converts semantic render lines into IOCraft row elements.
pub fn render_lines_elements(lines: Vec<RenderLine>) -> Vec<AnyElement<'static>> {
    lines.into_iter().map(render_line_element).collect()
}

/// Converts one adapter display line into one semantic render row.
pub fn display_line_render_line(line: &DisplayLine) -> RenderLine {
    RenderLine::styled(&line.text, RenderStyle::from(line.style))
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

/// Formats submitted prompt text as original marker rows.
pub fn submitted_prompt_render_lines(text: &str, style: RenderStyle) -> Vec<RenderLine> {
    let rows: Vec<&str> = text.lines().collect();
    if rows.is_empty() {
        return vec![RenderLine::styled("> ", style)];
    }

    rows.into_iter()
        .enumerate()
        .map(|(index, line)| {
            let marker = if index == 0 { "> " } else { "  " };
            RenderLine::styled(format!("{marker}{line}"), style)
        })
        .collect()
}

/// Splits non-empty text into visible rows without trimming row content.
fn visible_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }

    text.lines().map(ToOwned::to_owned).collect()
}

/// Splits text into visible rows only when it contains non-whitespace content.
fn visible_trimmed_lines(text: &str) -> Vec<String> {
    if text.trim().is_empty() {
        return Vec::new();
    }

    visible_lines(text)
}
