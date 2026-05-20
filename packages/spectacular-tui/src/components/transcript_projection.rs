use crate::components::assistant_message::assistant_message_render_lines;
use crate::components::cancellation::cancellation_render_lines;
use crate::components::command::{command_render_lines, command_row_count};
use crate::components::error::{error_render_lines, error_row_count};
use crate::components::notice::notice_render_lines;
use crate::components::opening_banner::opening_banner_render_lines;
use crate::components::reasoning::reasoning_render_lines;
use crate::components::success::success_render_lines;
use crate::components::tool_call::{tool_render_lines, tool_row_count};
use crate::components::transcript_content::{
    plain_lines, prompt_text_row_count, trimmed_visible_text_row_count, visible_text_row_count,
};
use crate::components::user_prompt::user_prompt_render_lines;
use crate::components::warning::warning_render_lines;
use crate::components::worked_summary::worked_summary_render_lines;
use crate::render_model::RenderLine;
use crate::state::State;
use crate::transcript::{TranscriptItem, TranscriptItemContent};
use std::ops::Range;
use unicode_width::UnicodeWidthChar;

/// Formats the semantic transcript region for legacy text assertions.
pub fn transcript_render_lines(state: &State) -> Vec<RenderLine> {
    transcript_render_lines_for_rows(
        state,
        crate::transcript_window::visible_transcript_row_count(&state.scroll),
    )
}

/// Formats the row-windowed transcript region for a known viewport height.
pub(crate) fn transcript_render_lines_for_rows(
    state: &State,
    visible_rows: usize,
) -> Vec<RenderLine> {
    if visible_rows == 0 {
        return Vec::new();
    }

    visible_transcript_rows(state, visible_rows)
}

/// Counts all rendered transcript rows before scroll windowing.
pub fn transcript_total_render_rows(state: &State) -> usize {
    state
        .session
        .transcript
        .iter()
        .map(transcript_item_row_count)
        .sum()
}

/// Row-aware transcript layout used by the live IOCraft virtualized transcript.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct TranscriptLayout {
    pub(crate) total_rows: usize,
    pub(crate) items: Vec<TranscriptItemLayout>,
}

impl TranscriptLayout {
    /// Builds cumulative row metadata for transcript items at a known content width.
    pub(crate) fn for_state(state: &State, width: usize) -> Self {
        let mut next_start_row = 0usize;
        let items = state
            .session
            .transcript
            .iter()
            .enumerate()
            .map(|(item_index, item)| {
                let row_count = transcript_item_row_count_for_width(item, width);
                let layout = TranscriptItemLayout {
                    item_index,
                    start_row: next_start_row,
                    row_count,
                };
                next_start_row = next_start_row.saturating_add(row_count);
                layout
            })
            .collect();

        Self {
            total_rows: next_start_row,
            items,
        }
    }

    /// Returns the item indices intersecting a half-open virtual row window.
    pub(crate) fn item_range(&self, rows: Range<usize>) -> Range<usize> {
        if rows.start >= rows.end || self.items.is_empty() {
            return 0..0;
        }

        let start = self
            .items
            .partition_point(|item| item.end_row() <= rows.start);
        let end = self.items.partition_point(|item| item.start_row < rows.end);

        start..end.max(start)
    }

    /// Returns the virtual row where an item starts, or zero for an empty range.
    pub(crate) fn item_start_row(&self, item_index: usize) -> usize {
        self.items
            .get(item_index)
            .map(|item| item.start_row)
            .unwrap_or_default()
    }
}

/// Cumulative row metadata for one semantic transcript item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TranscriptItemLayout {
    pub(crate) item_index: usize,
    pub(crate) start_row: usize,
    pub(crate) row_count: usize,
}

impl TranscriptItemLayout {
    /// Returns the first row after this item in the virtual transcript coordinate space.
    fn end_row(&self) -> usize {
        self.start_row.saturating_add(self.row_count)
    }
}

/// Returns the width-aware total row count used by live transcript layout.
pub fn transcript_layout_total_rows(state: &State, width: usize) -> usize {
    TranscriptLayout::for_state(state, width).total_rows
}

/// Returns the width-aware row starts used by live transcript layout.
pub fn transcript_layout_row_starts(state: &State, width: usize) -> Vec<usize> {
    TranscriptLayout::for_state(state, width)
        .items
        .into_iter()
        .map(|item| item.start_row)
        .collect()
}

/// Returns the item range intersecting a half-open virtual row window.
pub fn transcript_layout_item_range(
    state: &State,
    width: usize,
    rows: Range<usize>,
) -> Range<usize> {
    TranscriptLayout::for_state(state, width).item_range(rows)
}

/// Returns the width-aware row count used by live layout for one transcript item.
pub fn transcript_item_layout_rows(item: &TranscriptItem, width: usize) -> usize {
    transcript_item_row_count_for_width(item, width)
}

/// Returns the row count from the local IOCraft-style text wrapping model.
pub fn wrapped_layout_text_rows(text: &str, width: usize) -> usize {
    wrapped_text_row_count(text, width)
}

/// Formats the transcript region as plain visible text for compatibility tests.
pub fn transcript_lines(state: &State) -> Vec<String> {
    plain_lines(transcript_render_lines(state))
}

/// Formats one transcript item into one or more terminal-flow lines.
pub fn transcript_item_render_lines(item: &TranscriptItem) -> Vec<RenderLine> {
    match &item.content {
        TranscriptItemContent::OpeningBanner(banner) => opening_banner_render_lines(banner),
        TranscriptItemContent::UserPrompt(prompt) => user_prompt_render_lines(&prompt.text),
        TranscriptItemContent::AssistantMessage(message) => {
            assistant_message_render_lines(&message.text)
        }
        TranscriptItemContent::Reasoning(reasoning) => reasoning_render_lines(&reasoning.text),
        TranscriptItemContent::ToolCall(tool) => tool_render_lines(tool),
        TranscriptItemContent::Command(command) => command_render_lines(command),
        TranscriptItemContent::Error(error) => {
            error_render_lines(&error.message, error.details.as_deref())
        }
        TranscriptItemContent::Warning(warning) => warning_render_lines(&warning.message),
        TranscriptItemContent::Success(success) => success_render_lines(&success.message),
        TranscriptItemContent::Notice(notice) => notice_render_lines(&notice.message),
        TranscriptItemContent::Cancellation(cancellation) => {
            cancellation_render_lines(&cancellation.reason)
        }
        TranscriptItemContent::WorkedSummary(summary) => {
            worked_summary_render_lines(&summary.duration, summary.turn_tokens)
        }
    }
}

/// Formats one transcript item as plain visible text for compatibility tests.
pub fn transcript_item_lines(item: &TranscriptItem) -> Vec<String> {
    plain_lines(transcript_item_render_lines(item))
}

/// Counts rendered rows for one transcript item without materializing every row when possible.
fn transcript_item_row_count(item: &TranscriptItem) -> usize {
    match &item.content {
        TranscriptItemContent::OpeningBanner(_) => 7,
        TranscriptItemContent::UserPrompt(prompt) => prompt_text_row_count(&prompt.text),
        TranscriptItemContent::AssistantMessage(message) => visible_text_row_count(&message.text),
        TranscriptItemContent::Reasoning(reasoning) => {
            trimmed_visible_text_row_count(&reasoning.text)
        }
        TranscriptItemContent::ToolCall(tool) => tool_row_count(tool),
        TranscriptItemContent::Command(command) => command_row_count(command),
        TranscriptItemContent::Error(error) => error_row_count(error.details.as_deref()),
        TranscriptItemContent::Warning(_)
        | TranscriptItemContent::Success(_)
        | TranscriptItemContent::Notice(_)
        | TranscriptItemContent::Cancellation(_)
        | TranscriptItemContent::WorkedSummary(_) => 1,
    }
}

/// Estimates rendered rows for one transcript item after IOCraft text wrapping.
fn transcript_item_row_count_for_width(item: &TranscriptItem, width: usize) -> usize {
    if transcript_item_uses_no_wrap(item) {
        return transcript_item_row_count(item);
    }

    transcript_item_render_lines(item)
        .iter()
        .map(|line| wrapped_render_line_row_count(line, width))
        .sum()
}

/// Returns true when the IOCraft component renders each semantic row without wrapping.
fn transcript_item_uses_no_wrap(item: &TranscriptItem) -> bool {
    matches!(
        item.content,
        TranscriptItemContent::OpeningBanner(_)
            | TranscriptItemContent::ToolCall(_)
            | TranscriptItemContent::Command(_)
    )
}

/// Estimates wrapped terminal rows for one semantic render line.
fn wrapped_render_line_row_count(line: &RenderLine, width: usize) -> usize {
    if width == 0 {
        return 1;
    }

    let text = line.plain_text();
    wrapped_text_row_count(&text, width)
}

/// Counts rows for IOCraft-style wrapping using break opportunities after whitespace.
fn wrapped_text_row_count(text: &str, width: usize) -> usize {
    if text.is_empty() || width == 0 {
        return 1;
    }

    let mut rows = 1usize;
    let mut current_width = 0usize;
    for token in wrapping_tokens(text) {
        if current_width + token.non_trailing_width <= width {
            current_width = current_width.saturating_add(token.total_width);
            continue;
        }

        if current_width > 0 {
            rows = rows.saturating_add(1);
        }

        let (token_rows, token_width) = forced_wrap_width(token.non_trailing_width, width);
        rows = rows.saturating_add(token_rows.saturating_sub(1));
        current_width = token_width.saturating_add(token.trailing_width);
    }

    rows
}

/// Splits text into word-plus-trailing-whitespace units for local row estimation.
fn wrapping_tokens(text: &str) -> Vec<WrappingToken> {
    let mut tokens = Vec::new();
    let mut non_trailing_width = 0usize;
    let mut trailing_width = 0usize;

    for character in text.chars() {
        let width = character.width().unwrap_or(0);
        if character.is_whitespace() {
            trailing_width = trailing_width.saturating_add(width);
            continue;
        }

        if trailing_width > 0 && non_trailing_width > 0 {
            tokens.push(WrappingToken::new(non_trailing_width, trailing_width));
            non_trailing_width = 0;
            trailing_width = 0;
        }

        non_trailing_width = non_trailing_width.saturating_add(trailing_width);
        trailing_width = 0;
        non_trailing_width = non_trailing_width.saturating_add(width);
    }

    if non_trailing_width > 0 || trailing_width > 0 {
        tokens.push(WrappingToken::new(non_trailing_width, trailing_width));
    }

    tokens
}

/// Returns rows and final-row width after force-wrapping an unbreakable token.
fn forced_wrap_width(width: usize, row_width: usize) -> (usize, usize) {
    if width == 0 {
        return (1, 0);
    }

    let rows = width.saturating_add(row_width.saturating_sub(1)) / row_width;
    let remainder = width % row_width;
    (
        rows.max(1),
        if remainder == 0 { row_width } else { remainder },
    )
}

/// One local wrapping unit with whitespace that may be trimmed from fit decisions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WrappingToken {
    non_trailing_width: usize,
    trailing_width: usize,
    total_width: usize,
}

impl WrappingToken {
    /// Creates a wrapping token from non-trailing and trailing display widths.
    fn new(non_trailing_width: usize, trailing_width: usize) -> Self {
        Self {
            non_trailing_width,
            trailing_width,
            total_width: non_trailing_width.saturating_add(trailing_width),
        }
    }
}

/// Returns transcript rows that are within the current row-aware scroll window.
fn visible_transcript_rows(state: &State, visible_rows: usize) -> Vec<RenderLine> {
    let mut skipped_rows = state.scroll.offset as usize;
    let mut rows = Vec::with_capacity(visible_rows);

    for item in state.session.transcript.iter().rev() {
        let mut item_rows = transcript_item_render_lines(item);
        if skipped_rows >= item_rows.len() {
            skipped_rows = skipped_rows.saturating_sub(item_rows.len());
            continue;
        }

        if skipped_rows > 0 {
            item_rows.truncate(item_rows.len().saturating_sub(skipped_rows));
            skipped_rows = 0;
        }

        for row in item_rows.into_iter().rev() {
            rows.push(row);
            if rows.len() >= visible_rows {
                rows.reverse();
                return rows;
            }
        }
    }

    rows.reverse();
    rows
}
