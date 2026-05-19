pub use crate::format_directory::{format_directory, format_directory_with_home};
use crate::metadata::{CommandDescriptor, ContextTokenUsage};
use crate::render_model::{context_usage_style, RenderLine, RenderSpan, RenderStyle};
use crate::state::State;
use crate::status::Status;
use crate::transcript::{
    CommandItem, CommandStatus, DisplayLine, ToolCallItem, TranscriptItem, TranscriptItemContent,
};
use std::path::Path;
use unicode_width::UnicodeWidthStr;

const OPENING_BANNER_MIN_WIDTH: usize = 52;
const SEPARATOR: &str = " · ";

/// Formats the complete visible app projection as semantic render rows.
pub fn app_render_lines(state: &State) -> Vec<RenderLine> {
    let mut lines = transcript_render_lines(state);
    if let Some(working) = working_render_line(state) {
        lines.push(working);
    }
    lines.extend(prompt_render_lines(state));
    lines.push(footer_render_line(state));
    lines
}

/// Formats the complete visible app projection using original chat UI text shapes.
pub fn app_lines(state: &State) -> Vec<String> {
    plain_lines(app_render_lines(state))
}

/// Formats the semantic transcript region without prototype headings or placeholders.
pub fn transcript_render_lines(state: &State) -> Vec<RenderLine> {
    visible_transcript_items(state)
        .flat_map(transcript_item_render_lines)
        .collect()
}

/// Formats the transcript region as plain visible text for compatibility tests.
pub fn transcript_lines(state: &State) -> Vec<String> {
    plain_lines(transcript_render_lines(state))
}

/// Formats the active prompt with the original `> ` marker and multiline continuation.
pub fn prompt_render_lines(state: &State) -> Vec<RenderLine> {
    let mut lines = active_prompt_text_render_lines(state);
    lines.extend(slash_suggestion_render_lines(state));
    lines.extend(slash_usage_render_lines(state));
    lines
}

/// Formats the active prompt as plain visible text for compatibility tests.
pub fn prompt_lines(state: &State) -> Vec<String> {
    plain_lines(prompt_render_lines(state))
}

/// Formats footer metadata with original compact separators and optional usage.
pub fn footer_render_line(state: &State) -> RenderLine {
    let mut spans = vec![
        RenderSpan::new(
            format_directory(Path::new(&state.display.current_directory)),
            RenderStyle::Task,
        ),
        RenderSpan::new(SEPARATOR, RenderStyle::Dim),
        RenderSpan::new(&state.display.model_label, RenderStyle::Model),
        RenderSpan::new(
            format!(" ({})", state.display.reasoning_label),
            RenderStyle::Dim,
        ),
    ];
    if let Some(usage) = state.session.usage.or(state.display.usage) {
        spans.push(RenderSpan::new(SEPARATOR, RenderStyle::Dim));
        spans.push(RenderSpan::new(
            usage_text(usage),
            context_usage_style(usage),
        ));
    }

    RenderLine::from_spans(spans)
}

/// Formats footer metadata as plain visible text for compatibility tests.
pub fn footer_text(state: &State) -> String {
    footer_render_line(state).plain_text()
}

/// Formats optional context token usage for status/footer display.
pub fn usage_text(usage: ContextTokenUsage) -> String {
    let Some(window) = usage.context_window_tokens else {
        return format!("{} tks", compact_token_count(usage.input_tokens));
    };
    format!(
        "{}/{} tks",
        compact_token_count(usage.input_tokens),
        compact_token_count(window)
    )
}

/// Formats one transcript item into one or more terminal-flow lines.
pub fn transcript_item_render_lines(item: &TranscriptItem) -> Vec<RenderLine> {
    match &item.content {
        TranscriptItemContent::OpeningBanner(banner) => opening_banner_render_lines(banner),
        TranscriptItemContent::UserPrompt(prompt) => {
            prompt_text_render_lines(&prompt.text, RenderStyle::User)
        }
        TranscriptItemContent::AssistantMessage(message) => {
            styled_visible_lines(&message.text, RenderStyle::Assistant)
        }
        TranscriptItemContent::Reasoning(reasoning) => {
            styled_visible_trimmed_lines(&reasoning.text, RenderStyle::Reasoning)
        }
        TranscriptItemContent::ToolCall(tool) => tool_render_lines(tool),
        TranscriptItemContent::Command(command) => command_render_lines(command),
        TranscriptItemContent::Error(error) => {
            error_render_lines(&error.message, error.details.as_deref())
        }
        TranscriptItemContent::Warning(warning) => vec![RenderLine::styled(
            format!("warning: {}", warning.message),
            RenderStyle::Warning,
        )],
        TranscriptItemContent::Success(success) => {
            vec![RenderLine::styled(&success.message, RenderStyle::Success)]
        }
        TranscriptItemContent::Notice(notice) => vec![RenderLine::text(&notice.message)],
        TranscriptItemContent::Cancellation(cancellation) => {
            vec![RenderLine::styled(
                &cancellation.reason,
                RenderStyle::Warning,
            )]
        }
        TranscriptItemContent::WorkedSummary(summary) => vec![RenderLine::styled(
            worked_summary_text(&summary.duration, summary.turn_tokens),
            RenderStyle::Dim,
        )],
    }
}

/// Formats one transcript item as plain visible text for compatibility tests.
pub fn transcript_item_lines(item: &TranscriptItem) -> Vec<String> {
    plain_lines(transcript_item_render_lines(item))
}

/// Flattens semantic rows into plain visible text rows.
pub fn plain_lines(lines: Vec<RenderLine>) -> Vec<String> {
    lines.into_iter().map(|line| line.plain_text()).collect()
}

/// Returns transcript items that are within the current scroll window.
fn visible_transcript_items(state: &State) -> impl Iterator<Item = &TranscriptItem> {
    let range = crate::transcript_window::visible_transcript_range(
        state.session.transcript.len(),
        &state.scroll,
    );
    state.session.transcript[range].iter()
}

/// Formats the opening banner as original-width Unicode box drawing rows.
pub(crate) fn opening_banner_render_lines(
    banner: &crate::transcript::OpeningBannerItem,
) -> Vec<RenderLine> {
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
fn opening_banner_rows(banner: &crate::transcript::OpeningBannerItem) -> Vec<OpeningBannerRow> {
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

/// Formats one opening banner content row with green borders and styled inner text.
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

/// Formats the active or submitted prompt as original marker rows.
fn prompt_text_render_lines(text: &str, style: RenderStyle) -> Vec<RenderLine> {
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

/// Formats slash command suggestions under the active prompt.
fn slash_suggestion_render_lines(state: &State) -> Vec<RenderLine> {
    slash_suggestions(state)
        .into_iter()
        .enumerate()
        .map(|(index, command)| {
            slash_suggestion_render_line(command, index, state.session.prompt.selected_completion)
        })
        .collect()
}

/// Formats selected slash command usage guidance when available.
fn slash_usage_render_lines(state: &State) -> Vec<RenderLine> {
    let suggestions = slash_suggestions(state);
    let command = suggestions
        .get(state.session.prompt.selected_completion)
        .or_else(|| suggestions.first())
        .copied()
        .or_else(|| active_slash_command(state));
    let Some(command) = command else {
        return Vec::new();
    };
    if command.usage.is_empty() {
        return Vec::new();
    }

    vec![RenderLine::styled(&command.usage, RenderStyle::Dim)]
}

/// Formats a tool-call transcript item as original-shaped semantic rows.
fn tool_render_lines(tool: &ToolCallItem) -> Vec<RenderLine> {
    if let Some(display) = &tool.display {
        let mut lines = Vec::new();
        if let Some(call_line) = &display.call_line {
            lines.push(display_line_render_line(call_line));
        }
        lines.extend(display.argument_lines.iter().map(display_line_render_line));
        lines.extend(display.output_lines.iter().map(display_line_render_line));
        return lines;
    }

    let mut call = tool.name.clone();
    if let Some(arguments) = &tool.arguments_preview {
        if !arguments.trim().is_empty() {
            call.push(' ');
            call.push_str(arguments);
        }
    }

    let mut lines = vec![RenderLine::styled(call, RenderStyle::Tool)];
    lines.extend(styled_visible_lines(
        tool.output_preview.as_deref().unwrap_or_default(),
        RenderStyle::CommandOutput,
    ));
    lines
}

/// Formats a command transcript item as original-shaped semantic rows.
fn command_render_lines(command: &CommandItem) -> Vec<RenderLine> {
    if let Some(display) = &command.display {
        let mut lines = Vec::new();
        if let Some(command_line) = &display.command_line {
            lines.push(display_line_render_line(command_line));
        }
        lines.extend(display.output_lines.iter().map(display_line_render_line));
        if let Some(summary_line) = &display.summary_line {
            lines.push(display_line_render_line(summary_line));
        }
        return lines;
    }

    let mut lines = vec![RenderLine::styled(
        format!("$ {}", command.command),
        RenderStyle::Command,
    )];
    lines.extend(styled_visible_lines(
        &command.output,
        RenderStyle::CommandOutput,
    ));
    if command.status != CommandStatus::Failed {
        return lines;
    }
    if let Some(exit_code) = command.exit_code {
        lines.push(RenderLine::styled(
            format!("exit: {exit_code}"),
            RenderStyle::Error,
        ));
    }
    lines
}

/// Converts one adapter display line into one semantic render row.
fn display_line_render_line(line: &DisplayLine) -> RenderLine {
    RenderLine::styled(&line.text, RenderStyle::from(line.style))
}

/// Formats an error transcript item as original-shaped semantic rows.
fn error_render_lines(message: &str, details: Option<&str>) -> Vec<RenderLine> {
    let mut lines = vec![RenderLine::styled(
        format!("error: {message}"),
        RenderStyle::Error,
    )];
    if let Some(details) = details {
        lines.extend(styled_visible_lines(details, RenderStyle::CommandOutput));
    }
    lines
}

/// Formats visible lines with one semantic style per row.
fn styled_visible_lines(text: &str, style: RenderStyle) -> Vec<RenderLine> {
    visible_lines(text)
        .into_iter()
        .map(|line| RenderLine::styled(line, style))
        .collect()
}

/// Formats non-blank visible lines with one semantic style per row.
fn styled_visible_trimmed_lines(text: &str, style: RenderStyle) -> Vec<RenderLine> {
    visible_trimmed_lines(text)
        .into_iter()
        .map(|line| RenderLine::styled(line, style))
        .collect()
}

/// Formats the active prompt with semantic spans for selected text ranges.
fn active_prompt_text_render_lines(state: &State) -> Vec<RenderLine> {
    let text = &state.session.prompt.text;
    let rows: Vec<&str> = text.lines().collect();
    if rows.is_empty() {
        return vec![RenderLine::styled("> ", RenderStyle::User)];
    }

    let mut offset = 0;
    rows.into_iter()
        .enumerate()
        .map(|(index, line)| {
            let marker = if index == 0 { "> " } else { "  " };
            let line_start = offset;
            let line_end = line_start + line.len();
            offset = line_end + 1;
            selected_prompt_render_line(
                marker,
                line,
                line_start,
                line_end,
                state.session.prompt.selection_range(),
            )
        })
        .collect()
}

/// Formats one prompt row, highlighting the intersection with active selection.
fn selected_prompt_render_line(
    marker: &str,
    line: &str,
    line_start: usize,
    line_end: usize,
    selection: Option<std::ops::Range<usize>>,
) -> RenderLine {
    let Some(selection) = selection else {
        return RenderLine::styled(format!("{marker}{line}"), RenderStyle::User);
    };

    let start = selection.start.max(line_start);
    let end = selection.end.min(line_end);
    if start >= end {
        return RenderLine::styled(format!("{marker}{line}"), RenderStyle::User);
    }

    let local_start = start - line_start;
    let local_end = end - line_start;
    let mut spans = vec![RenderSpan::new(marker, RenderStyle::User)];
    spans.push(RenderSpan::new(&line[..local_start], RenderStyle::User));
    spans.push(RenderSpan::new(
        &line[local_start..local_end],
        RenderStyle::Selection,
    ));
    spans.push(RenderSpan::new(&line[local_end..], RenderStyle::User));
    RenderLine::from_spans(spans)
}

/// Returns display-ready slash command suggestions for the active prompt.
fn slash_suggestions(state: &State) -> Vec<&CommandDescriptor> {
    crate::prompt_state::slash_suggestions(&state.session.prompt, &state.commands)
}

/// Returns the accepted leading slash command when the prompt has command arguments.
fn active_slash_command(state: &State) -> Option<&CommandDescriptor> {
    let text = state.session.prompt.text.trim_start();
    let command_name = text.strip_prefix('/')?.split_whitespace().next()?;
    if !text[command_name.len() + 1..].starts_with(char::is_whitespace) {
        return None;
    }

    state
        .commands
        .iter()
        .find(|command| command.name == command_name)
}

/// Formats one slash suggestion row using original padding and selection styles.
fn slash_suggestion_render_line(
    command: &CommandDescriptor,
    index: usize,
    selected: usize,
) -> RenderLine {
    let style = if index == selected {
        RenderStyle::User
    } else {
        RenderStyle::Dim
    };
    let label = format!("/{}", command.name);
    RenderLine::styled(format!("  {label:<18} {}", command.summary), style)
}

/// Formats the current working status line as a semantic row when active.
pub fn working_render_line(state: &State) -> Option<RenderLine> {
    match &state.status {
        Status::Running { .. } | Status::Cancelling => Some(RenderLine::styled(
            format!(
                "{} Working (CTRL + C to stop)",
                state.spinner.current_frame()
            ),
            RenderStyle::Dim,
        )),
        Status::Idle | Status::Failed { .. } => None,
    }
}

/// Formats a completed work summary with duration and turn-token count.
fn worked_summary_text(duration: &str, turn_tokens: Option<u64>) -> String {
    format!(
        "Worked for {duration}{SEPARATOR}total {} tokens",
        turn_tokens.unwrap_or(0)
    )
}

/// Formats token counts with compact `k` suffixes for whole thousands.
fn compact_token_count(tokens: u64) -> String {
    if tokens < 1_000 {
        return tokens.to_string();
    }

    format!("{}k", tokens / 1_000)
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
