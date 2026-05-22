use crate::metadata::CommandDescriptor;
use crate::prompt::layout::{row_for_cursor, visual_rows, VisualRow};
use crate::prompt::{CommandGuidanceLine, CommandSuggestion, CommandSuggestionKind};
use crate::render::{RenderLine, RenderSpan, RenderStyle};
use crate::session::PromptState;
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

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
    let suggestions = crate::prompt::command_suggestions(prompt, commands);
    let mut guidance = command_guidance_lines(prompt, commands);
    if guidance.is_empty() {
        guidance = slash_usage_lines(prompt, commands, &suggestions);
    }
    let has_guidance = !guidance.is_empty();
    let has_suggestions = !suggestions.is_empty();

    lines.extend(guidance);
    if has_guidance && has_suggestions {
        lines.push(RenderLine::styled("", RenderStyle::Dim));
    }
    lines.extend(slash_suggestion_lines(prompt, &suggestions));
    lines
}

/// Formats prompt text rows, including placeholder, selection, and cursor spans.
fn prompt_text_lines(prompt: &PromptState, width: Option<u16>) -> Vec<RenderLine> {
    if prompt.is_empty() {
        return vec![empty_prompt_line()];
    }

    let text = prompt.text();
    let content_width = content_width(width);
    let rows = visual_rows(&text, content_width);
    let cursor_row = row_for_cursor(&rows, prompt.cursor);
    rows.into_iter()
        .enumerate()
        .skip(prompt.scroll_top_row)
        .map(|(index, row)| {
            let marker = if index == 0 {
                PROMPT_MARKER
            } else {
                CONTINUATION_MARKER
            };
            let cursor = (index == cursor_row).then_some(prompt.cursor);
            prompt_row_line(marker, &text, row, prompt, cursor)
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

    let grapheme = text[cursor..row.end].graphemes(true).next()?;
    Some(cursor..cursor + grapheme.len())
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
fn slash_suggestion_lines(
    prompt: &PromptState,
    suggestions: &[CommandSuggestion],
) -> Vec<RenderLine> {
    suggestions
        .into_iter()
        .enumerate()
        .map(|(index, suggestion)| {
            slash_suggestion_line(suggestion, index, prompt.selected_completion)
        })
        .collect()
}

/// Formats structured command-composer guidance rows.
fn command_guidance_lines(prompt: &PromptState, commands: &[CommandDescriptor]) -> Vec<RenderLine> {
    crate::prompt::command_guidance(prompt, commands)
        .into_iter()
        .map(command_guidance_line)
        .collect()
}

/// Formats one command-composer guidance row.
fn command_guidance_line(line: CommandGuidanceLine) -> RenderLine {
    match line {
        CommandGuidanceLine::Missing(fields) => RenderLine::styled(
            format!("missing: {}.", fields.join(", ")),
            RenderStyle::Secret,
        ),
        CommandGuidanceLine::Info(value) => RenderLine::styled(value, RenderStyle::Secret),
        CommandGuidanceLine::Detail(value) => RenderLine::styled(value, RenderStyle::Dim),
    }
}

/// Formats selected slash command usage guidance when available.
fn slash_usage_lines(
    prompt: &PromptState,
    commands: &[CommandDescriptor],
    suggestions: &[CommandSuggestion],
) -> Vec<RenderLine> {
    let command = suggestions
        .get(prompt.selected_completion)
        .or_else(|| suggestions.first())
        .and_then(|suggestion| command_for_suggestion(suggestion, commands))
        .or_else(|| active_slash_command(prompt, commands));
    let Some(command) = command else {
        return Vec::new();
    };
    if command.usage.is_empty() {
        return Vec::new();
    }

    vec![RenderLine::styled(&command.usage, RenderStyle::Dim)]
}

/// Returns the command descriptor represented by a top-level command suggestion.
fn command_for_suggestion<'a>(
    suggestion: &CommandSuggestion,
    commands: &'a [CommandDescriptor],
) -> Option<&'a CommandDescriptor> {
    if suggestion.kind != CommandSuggestionKind::Command {
        return None;
    }

    commands
        .iter()
        .find(|command| command.name == suggestion.replacement)
}

/// Returns the accepted leading slash command when the prompt has command arguments.
fn active_slash_command<'a>(
    prompt: &PromptState,
    commands: &'a [CommandDescriptor],
) -> Option<&'a CommandDescriptor> {
    let prompt_text = prompt.text();
    let command_text = prompt_text.strip_prefix('/')?;
    let command_name = command_text.split_whitespace().next()?;
    let arguments_start = command_name.len();
    let arguments = command_text.get(arguments_start..)?;
    if !arguments.starts_with(char::is_whitespace) {
        return None;
    }

    commands.iter().find(|command| command.name == command_name)
}

/// Formats one slash suggestion row using original padding and selection styles.
fn slash_suggestion_line(
    suggestion: &CommandSuggestion,
    index: usize,
    selected: usize,
) -> RenderLine {
    let style = if index == selected && suggestion.kind != CommandSuggestionKind::Info {
        RenderStyle::User
    } else {
        RenderStyle::Dim
    };
    RenderLine::styled(
        format!("  {:<18} {}", suggestion.label, suggestion.summary),
        style,
    )
}
