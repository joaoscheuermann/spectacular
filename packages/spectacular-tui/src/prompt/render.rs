use crate::metadata::CommandDescriptor;
use crate::prompt::layout::{row_for_cursor, visual_rows, VisualRow};
use crate::render::{RenderLine, RenderSpan, RenderStyle};
use crate::session::PromptState;
use std::ops::Range;

pub(crate) const PLACEHOLDER: &str = "What we are going to build today?";
const CURSOR_CELL: &str = " ";
const PROMPT_MARKER: &str = "> ";
const CONTINUATION_MARKER: &str = "  ";
const PROMPT_MARKER_WIDTH: usize = 2;
const DEFAULT_CONTENT_WIDTH: usize = 120;

/// Formats the active prompt and contextual slash command help.
pub(crate) fn render_lines(
    prompt: &PromptState,
    commands: &[CommandDescriptor],
    width: Option<u16>,
) -> Vec<RenderLine> {
    let mut lines = prompt_text_lines(prompt, width);
    lines.extend(slash_suggestion_lines(prompt, commands));
    lines.extend(slash_usage_lines(prompt, commands));
    lines
}

/// Formats prompt text rows, including placeholder, selection, and cursor spans.
fn prompt_text_lines(prompt: &PromptState, width: Option<u16>) -> Vec<RenderLine> {
    if prompt.text.is_empty() {
        return vec![empty_prompt_line()];
    }

    let content_width = content_width(width);
    let rows = visual_rows(&prompt.text, content_width);
    let cursor_row = row_for_cursor(&rows, prompt.cursor);
    rows.into_iter()
        .enumerate()
        .map(|(index, row)| {
            let marker = if index == 0 {
                PROMPT_MARKER
            } else {
                CONTINUATION_MARKER
            };
            let cursor = (index == cursor_row).then_some(prompt.cursor);
            prompt_row_line(marker, &prompt.text, row, prompt, cursor)
        })
        .collect()
}

/// Formats the placeholder row for an empty prompt.
fn empty_prompt_line() -> RenderLine {
    RenderLine::from_spans(vec![
        RenderSpan::new(PROMPT_MARKER, RenderStyle::User),
        RenderSpan::new(CURSOR_CELL, RenderStyle::Selection),
        RenderSpan::new(PLACEHOLDER, RenderStyle::Dim),
    ])
}

/// Formats one visual prompt row.
fn prompt_row_line(
    marker: &str,
    text: &str,
    row: VisualRow,
    prompt: &PromptState,
    cursor: Option<usize>,
) -> RenderLine {
    let mut spans = vec![RenderSpan::new(marker, RenderStyle::User)];
    push_text_with_selection_and_cursor(&mut spans, text, row, prompt.selection_range(), cursor);
    RenderLine::from_spans(spans)
}

/// Appends row text with selected and current-cursor cells highlighted.
fn push_text_with_selection_and_cursor(
    spans: &mut Vec<RenderSpan>,
    text: &str,
    row: VisualRow,
    selection: Option<Range<usize>>,
    cursor: Option<usize>,
) {
    let selection = selection.and_then(|range| intersect_range(range, row.start..row.end));
    let cursor_range = cursor.and_then(|cursor| cursor_cell_range(text, row, cursor));
    let boundaries = segment_boundaries(row, selection.clone(), cursor_range.clone());

    for pair in boundaries.windows(2) {
        let range = pair[0]..pair[1];
        let style = segment_style(&range, selection.as_ref(), cursor_range.as_ref());
        push_segment(spans, text, range, style);
    }

    if cursor == Some(row.end) {
        spans.push(RenderSpan::new(CURSOR_CELL, RenderStyle::Selection));
    }
}

/// Appends a non-empty text segment with the supplied semantic style.
fn push_segment(spans: &mut Vec<RenderSpan>, text: &str, range: Range<usize>, style: RenderStyle) {
    if range.is_empty() {
        return;
    }

    spans.push(RenderSpan::new(&text[range], style));
}

/// Returns the byte range for the character occupying the cursor cell.
fn cursor_cell_range(text: &str, row: VisualRow, cursor: usize) -> Option<Range<usize>> {
    if cursor < row.start || cursor >= row.end || !text.is_char_boundary(cursor) {
        return None;
    }

    let character = text[cursor..row.end].chars().next()?;
    Some(cursor..cursor + character.len_utf8())
}

/// Returns sorted split points needed to apply text, selection, and cursor styles.
fn segment_boundaries(
    row: VisualRow,
    selection: Option<Range<usize>>,
    cursor: Option<Range<usize>>,
) -> Vec<usize> {
    let mut boundaries = vec![row.start, row.end];
    if let Some(range) = selection {
        boundaries.extend([range.start, range.end]);
    }
    if let Some(range) = cursor {
        boundaries.extend([range.start, range.end]);
    }

    boundaries.sort_unstable();
    boundaries.dedup();
    boundaries
}

/// Returns the style for a row segment after selection and cursor splitting.
fn segment_style(
    range: &Range<usize>,
    selection: Option<&Range<usize>>,
    cursor: Option<&Range<usize>>,
) -> RenderStyle {
    if overlaps(range, cursor) || overlaps(range, selection) {
        return RenderStyle::Selection;
    }

    RenderStyle::Text
}

/// Returns whether a segment overlaps an optional styled range.
fn overlaps(range: &Range<usize>, styled: Option<&Range<usize>>) -> bool {
    let Some(styled) = styled else {
        return false;
    };

    range.start < styled.end && styled.start < range.end
}

/// Returns the overlap between two byte ranges.
fn intersect_range(left: Range<usize>, right: Range<usize>) -> Option<Range<usize>> {
    let start = left.start.max(right.start);
    let end = left.end.min(right.end);
    if start >= end {
        return None;
    }

    Some(start..end)
}

/// Returns the prompt content width after reserving marker cells.
fn content_width(width: Option<u16>) -> usize {
    width
        .map(usize::from)
        .unwrap_or(DEFAULT_CONTENT_WIDTH)
        .saturating_sub(PROMPT_MARKER_WIDTH)
        .max(1)
}

/// Formats slash command suggestions under the active prompt.
fn slash_suggestion_lines(prompt: &PromptState, commands: &[CommandDescriptor]) -> Vec<RenderLine> {
    slash_suggestions(prompt, commands)
        .into_iter()
        .enumerate()
        .map(|(index, command)| slash_suggestion_line(command, index, prompt.selected_completion))
        .collect()
}

/// Formats selected slash command usage guidance when available.
fn slash_usage_lines(prompt: &PromptState, commands: &[CommandDescriptor]) -> Vec<RenderLine> {
    let suggestions = slash_suggestions(prompt, commands);
    let command = suggestions
        .get(prompt.selected_completion)
        .or_else(|| suggestions.first())
        .copied()
        .or_else(|| active_slash_command(prompt, commands));
    let Some(command) = command else {
        return Vec::new();
    };
    if command.usage.is_empty() {
        return Vec::new();
    }

    vec![RenderLine::styled(&command.usage, RenderStyle::Dim)]
}

/// Returns display-ready slash command suggestions for the active prompt.
fn slash_suggestions<'a>(
    prompt: &PromptState,
    commands: &'a [CommandDescriptor],
) -> Vec<&'a CommandDescriptor> {
    crate::prompt::slash_suggestions(prompt, commands)
}

/// Returns the accepted leading slash command when the prompt has command arguments.
fn active_slash_command<'a>(
    prompt: &PromptState,
    commands: &'a [CommandDescriptor],
) -> Option<&'a CommandDescriptor> {
    let text = prompt.text.trim_start();
    let command_name = text.strip_prefix('/')?.split_whitespace().next()?;
    if !text[command_name.len() + 1..].starts_with(char::is_whitespace) {
        return None;
    }

    commands.iter().find(|command| command.name == command_name)
}

/// Formats one slash suggestion row using original padding and selection styles.
fn slash_suggestion_line(command: &CommandDescriptor, index: usize, selected: usize) -> RenderLine {
    let style = if index == selected {
        RenderStyle::User
    } else {
        RenderStyle::Dim
    };
    let label = format!("/{}", command.name);
    RenderLine::styled(format!("  {label:<18} {}", command.summary), style)
}
