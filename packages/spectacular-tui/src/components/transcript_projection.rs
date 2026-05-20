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
use unicode_width::UnicodeWidthStr;

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

/// Estimates laid-out transcript rows for a known wrapping width.
pub fn transcript_total_render_rows_for_width(state: &State, width: usize) -> usize {
    if width == 0 {
        return transcript_total_render_rows(state);
    }

    state
        .session
        .transcript
        .iter()
        .map(|item| transcript_item_row_count_for_width(item, width))
        .sum()
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
    transcript_item_render_lines(item)
        .iter()
        .map(|line| wrapped_render_line_row_count(line, width))
        .sum()
}

/// Estimates wrapped terminal rows for one semantic render line.
fn wrapped_render_line_row_count(line: &RenderLine, width: usize) -> usize {
    let visible_width: usize = line
        .spans
        .iter()
        .map(|span| UnicodeWidthStr::width(span.text.as_str()))
        .sum();

    visible_width.checked_div(width).unwrap_or(0)
        + usize::from(visible_width % width != 0 || visible_width == 0)
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
