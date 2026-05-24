use super::model::{
    CommandItem, CommandStatus, DisplayLine, OpeningBannerItem, ToolCallItem, TranscriptItem,
    TranscriptItemContent,
};
use crate::render::format_directory;
use std::path::Path;
use unicode_width::UnicodeWidthStr;

const OPENING_BANNER_MIN_WIDTH: usize = 52;
const TRANSCRIPT_SEPARATOR: &str = " \u{00b7} ";

/// Wrapping behavior for one semantic transcript row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TranscriptRowWrap {
    Wrap,
    NoWrap,
}

/// Plain transcript row used by layout and selection hit-testing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TranscriptRow {
    pub(crate) line: usize,
    pub(crate) text: String,
    pub(crate) wrap: TranscriptRowWrap,
}

impl TranscriptRow {
    fn new(line: usize, text: impl Into<String>, wrap: TranscriptRowWrap) -> Self {
        Self {
            line,
            text: text.into(),
            wrap,
        }
    }
}

/// Returns semantic rows for one transcript item, including the inter-item separator row.
pub(crate) fn transcript_item_rows(item: &TranscriptItem) -> Vec<TranscriptRow> {
    let wrap = if transcript_item_uses_no_wrap(item) {
        TranscriptRowWrap::NoWrap
    } else {
        TranscriptRowWrap::Wrap
    };
    let mut rows = transcript_item_content_rows(item)
        .into_iter()
        .enumerate()
        .map(|(line, text)| TranscriptRow::new(line, text, wrap))
        .collect::<Vec<_>>();
    rows.push(TranscriptRow::new(rows.len(), String::new(), wrap));
    rows
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

fn transcript_item_content_rows(item: &TranscriptItem) -> Vec<String> {
    match &item.content {
        TranscriptItemContent::OpeningBanner(banner) => opening_banner_rows(banner),
        TranscriptItemContent::UserPrompt(prompt) => visible_lines(&prompt.text),
        TranscriptItemContent::AssistantMessage(message) => visible_lines(&message.text),
        TranscriptItemContent::Reasoning(reasoning) => visible_trimmed_lines(&reasoning.text),
        TranscriptItemContent::ToolCall(tool) => tool_rows(tool),
        TranscriptItemContent::Command(command) => command_rows(command),
        TranscriptItemContent::Error(error) => error_rows(&error.message, error.details.as_deref()),
        TranscriptItemContent::Warning(warning) => vec![format!("warning: {}", warning.message)],
        TranscriptItemContent::Success(success) => vec![success.message.clone()],
        TranscriptItemContent::Notice(notice) => vec![notice.message.clone()],
        TranscriptItemContent::Cancellation(cancellation) => vec![cancellation.reason.clone()],
        TranscriptItemContent::WorkedSummary(summary) => {
            vec![worked_summary_row(&summary.duration, summary.turn_tokens)]
        }
    }
}

fn opening_banner_rows(banner: &OpeningBannerItem) -> Vec<String> {
    let rows = vec![
        format!("Spectacular (v{})", banner.version),
        String::new(),
        format!("model:     {} {}", banner.model, banner.reasoning),
        format!(
            "directory: {}",
            format_directory(Path::new(&banner.directory))
        ),
        format!("session:   {}", banner.session_id),
    ];
    let content_width = rows
        .iter()
        .map(|row| UnicodeWidthStr::width(row.as_str()))
        .max()
        .unwrap_or_default()
        .max(OPENING_BANNER_MIN_WIDTH);
    let horizontal = "\u{2500}".repeat(content_width + 2);
    let mut output = vec![format!("\u{256d}{horizontal}\u{256e}")];
    output.extend(rows.into_iter().map(|row| {
        let padding = content_width.saturating_sub(UnicodeWidthStr::width(row.as_str()));
        format!("\u{2502} {row}{} \u{2502}", " ".repeat(padding))
    }));
    output.push(format!("\u{2570}{horizontal}\u{256f}"));
    output
}

fn tool_rows(tool: &ToolCallItem) -> Vec<String> {
    if let Some(display) = &tool.display {
        let mut rows = Vec::new();
        push_display_line(&mut rows, display.call_line.as_ref());
        rows.extend(display.argument_lines.iter().map(display_text));
        rows.extend(display.output_lines.iter().map(display_text));
        return rows;
    }

    let mut call = tool.name.clone();
    if let Some(arguments) = &tool.arguments_preview {
        if !arguments.trim().is_empty() {
            call.push(' ');
            call.push_str(arguments);
        }
    }

    let mut rows = vec![call];
    rows.extend(
        tool.output_preview
            .as_deref()
            .map(visible_lines)
            .unwrap_or_default(),
    );
    rows
}

fn command_rows(command: &CommandItem) -> Vec<String> {
    if let Some(display) = &command.display {
        let mut rows = Vec::new();
        push_display_line(&mut rows, display.command_line.as_ref());
        rows.extend(display.output_lines.iter().map(display_text));
        push_display_line(&mut rows, display.summary_line.as_ref());
        return rows;
    }

    let mut rows = vec![format!("$ {}", command.command)];
    rows.extend(visible_lines(&command.output));
    if command.status == CommandStatus::Failed {
        if let Some(exit_code) = command.exit_code {
            rows.push(format!("exit: {exit_code}"));
        }
    }
    rows
}

fn error_rows(message: &str, details: Option<&str>) -> Vec<String> {
    let mut rows = vec![format!("error: {message}")];
    if let Some(details) = details {
        rows.extend(visible_lines(details));
    }
    rows
}

fn worked_summary_row(duration: &str, turn_tokens: Option<u64>) -> String {
    format!(
        "Worked for {duration}{TRANSCRIPT_SEPARATOR}total {} tokens",
        turn_tokens.unwrap_or_default()
    )
}

fn push_display_line(rows: &mut Vec<String>, line: Option<&DisplayLine>) {
    if let Some(line) = line {
        rows.push(display_text(line));
    }
}

fn display_text(line: &DisplayLine) -> String {
    line.text.clone()
}

fn visible_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }

    text.lines().map(ToOwned::to_owned).collect()
}

fn visible_trimmed_lines(text: &str) -> Vec<String> {
    if text.trim().is_empty() {
        return Vec::new();
    }

    visible_lines(text)
}
