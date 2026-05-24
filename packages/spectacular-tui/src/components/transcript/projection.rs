use super::assistant::assistant_message_render_lines;
use super::banner::opening_banner_render_lines;
use super::cancellation::cancellation_render_lines;
use super::command::{command_render_lines, command_row_count};
use super::content::{plain_lines, trimmed_visible_text_row_count, visible_text_row_count};
use super::error::{error_render_lines, error_row_count};
use super::notice::notice_render_lines;
use super::reasoning::reasoning_render_lines;
use super::success::success_render_lines;
use super::summary::worked_summary_render_lines;
use super::tool::{tool_render_lines, tool_row_count};
use super::user::user_prompt_render_lines;
use super::warning::warning_render_lines;
use crate::render::{RenderLine, RenderStyle};
use crate::state::State;
use crate::transcript::{
    transcript_item_row_count_for_width, wrapped_layout_text_rows as layout_text_rows,
    TranscriptItem, TranscriptItemContent, TranscriptLayout,
};
use std::ops::Range;

/// Formats the semantic transcript region for legacy text assertions.
pub fn transcript_render_lines(state: &State) -> Vec<RenderLine> {
    transcript_render_lines_for_rows(state, crate::transcript::visible_row_count(&state.scroll))
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
    layout_text_rows(text, width)
}

/// Formats the transcript region as plain visible text for compatibility tests.
pub fn transcript_lines(state: &State) -> Vec<String> {
    plain_lines(transcript_render_lines(state))
}

/// Formats one transcript item into one or more terminal-flow lines.
pub fn transcript_item_render_lines(item: &TranscriptItem) -> Vec<RenderLine> {
    let mut lines = transcript_item_content_render_lines(item);
    lines.push(RenderLine::styled("", RenderStyle::Text));
    lines
}

/// Formats one transcript item as plain visible text for compatibility tests.
pub fn transcript_item_lines(item: &TranscriptItem) -> Vec<String> {
    plain_lines(transcript_item_render_lines(item))
}

/// Formats one transcript item into its content rows without layout margins.
fn transcript_item_content_render_lines(item: &TranscriptItem) -> Vec<RenderLine> {
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

/// Counts rendered rows for one transcript item without materializing every row when possible.
fn transcript_item_row_count(item: &TranscriptItem) -> usize {
    match &item.content {
        TranscriptItemContent::OpeningBanner(_) => 8,
        TranscriptItemContent::UserPrompt(prompt) => {
            visible_text_row_count(&prompt.text).saturating_add(1)
        }
        TranscriptItemContent::AssistantMessage(message) => {
            visible_text_row_count(&message.text).saturating_add(1)
        }
        TranscriptItemContent::Reasoning(reasoning) => {
            trimmed_visible_text_row_count(&reasoning.text).saturating_add(1)
        }
        TranscriptItemContent::ToolCall(tool) => tool_row_count(tool).saturating_add(1),
        TranscriptItemContent::Command(command) => command_row_count(command).saturating_add(1),
        TranscriptItemContent::Error(error) => {
            error_row_count(error.details.as_deref()).saturating_add(1)
        }
        TranscriptItemContent::Warning(_)
        | TranscriptItemContent::Success(_)
        | TranscriptItemContent::Notice(_)
        | TranscriptItemContent::Cancellation(_)
        | TranscriptItemContent::WorkedSummary(_) => 2,
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
