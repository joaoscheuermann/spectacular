use crate::metadata::ContextTokenUsage;
use crate::state::State;
use crate::status::Status;
use crate::transcript::{CommandItem, CommandStatus, ToolCallItem, TranscriptItem, TranscriptItemContent};
use std::path::{Path, PathBuf, MAIN_SEPARATOR};
use unicode_width::UnicodeWidthStr;

const OPENING_BANNER_MIN_WIDTH: usize = 52;

/// Formats the complete visible app projection using original chat UI text shapes.
pub fn app_lines(state: &State) -> Vec<String> {
    let mut lines = transcript_lines(state);
    if let Some(working) = working_line(state) {
        lines.push(working);
    }
    lines.extend(prompt_lines(state));
    lines.push(footer_text(state));
    lines
}

/// Formats the semantic transcript region without prototype headings or placeholders.
pub fn transcript_lines(state: &State) -> Vec<String> {
    visible_transcript_items(state)
        .flat_map(transcript_item_lines)
        .collect()
}

/// Formats the active prompt with the original `> ` marker and multiline continuation.
pub fn prompt_lines(state: &State) -> Vec<String> {
    let prompt_text = &state.session.prompt.text;
    let mut lines = if prompt_text.is_empty() {
        vec!["> ".to_owned()]
    } else {
        prompt_text
            .split('\n')
            .enumerate()
            .map(|(index, line)| {
                if index == 0 {
                    format!("> {line}")
                } else {
                    format!("  {line}")
                }
            })
            .collect()
    };

    lines.extend(slash_suggestion_lines(state));
    lines
}

/// Formats footer metadata with original compact separators and optional usage.
pub fn footer_text(state: &State) -> String {
    let mut text = format!(
        "{} · {} ({})",
        format_directory(Path::new(&state.display.current_directory)),
        state.display.model_label,
        state.display.reasoning_label
    );
    if let Some(usage) = state.session.usage.or(state.display.usage) {
        text.push_str(" · ");
        text.push_str(&usage_text(usage));
    }
    text
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
pub fn transcript_item_lines(item: &TranscriptItem) -> Vec<String> {
    match &item.content {
        TranscriptItemContent::OpeningBanner(banner) => format_opening_banner(banner),
        TranscriptItemContent::UserPrompt(prompt) => visible_lines(&prompt.text),
        TranscriptItemContent::AssistantMessage(message) => visible_lines(&message.text),
        TranscriptItemContent::Reasoning(reasoning) => visible_trimmed_lines(&reasoning.text),
        TranscriptItemContent::ToolCall(tool) => tool_lines(tool),
        TranscriptItemContent::Command(command) => command_lines(command),
        TranscriptItemContent::Error(error) => error_lines(&error.message, error.details.as_deref()),
        TranscriptItemContent::Warning(warning) => vec![format!("warning: {}", warning.message)],
        TranscriptItemContent::Success(success) => vec![success.message.clone()],
        TranscriptItemContent::Notice(notice) => vec![notice.message.clone()],
        TranscriptItemContent::Cancellation(cancellation) => vec![cancellation.reason.clone()],
        TranscriptItemContent::WorkedSummary(summary) => {
            vec![worked_summary_text(&summary.duration, summary.turn_tokens)]
        }
    }
}

fn visible_transcript_items(state: &State) -> impl Iterator<Item = &TranscriptItem> {
    let range = crate::transcript_window::visible_transcript_range(
        state.session.transcript.len(),
        &state.scroll,
    );
    state.session.transcript[range].iter()
}

fn format_opening_banner(banner: &crate::transcript::OpeningBannerItem) -> Vec<String> {
    let title = format!("Spectacular (v{})", banner.version);
    let spacer = String::new();
    let model = format!("model:     {} {}", banner.model, banner.reasoning);
    let directory = format!("directory: {}", format_directory(Path::new(&banner.directory)));
    let session = format!("session:   {}", banner.session_id);
    let rows = [&title, &spacer, &model, &directory, &session];
    let content_width = rows
        .iter()
        .map(|line| UnicodeWidthStr::width(line.as_str()))
        .max()
        .unwrap_or(0)
        .max(OPENING_BANNER_MIN_WIDTH);
    let horizontal = "─".repeat(content_width + 2);

    let mut lines = vec![format!("┌{horizontal}┐")];
    lines.extend(rows.iter().map(|line| {
        format!("│ {} │", pad_banner_line(line, content_width))
    }));
    lines.push(format!("└{horizontal}┘"));
    lines
}

fn pad_banner_line(line: &str, width: usize) -> String {
    let padding = width.saturating_sub(UnicodeWidthStr::width(line));
    format!("{line}{}", " ".repeat(padding))
}

fn tool_lines(tool: &ToolCallItem) -> Vec<String> {
    let mut lines = Vec::new();
    let mut call = tool.name.clone();
    if let Some(arguments) = &tool.arguments_preview {
        if !arguments.trim().is_empty() {
            call.push(' ');
            call.push_str(arguments);
        }
    }
    lines.push(call);
    if let Some(output) = &tool.output_preview {
        lines.extend(visible_lines(output));
    }
    lines
}

fn command_lines(command: &CommandItem) -> Vec<String> {
    let mut lines = vec![format!("$ {}", command.command)];
    lines.extend(visible_lines(&command.output));
    if command.status == CommandStatus::Failed {
        if let Some(exit_code) = command.exit_code {
            lines.push(format!("exit: {exit_code}"));
        }
    }
    lines
}

fn error_lines(message: &str, details: Option<&str>) -> Vec<String> {
    let mut lines = vec![format!("error: {message}")];
    if let Some(details) = details {
        lines.extend(visible_lines(details));
    }
    lines
}

fn slash_suggestion_lines(state: &State) -> Vec<String> {
    let Some(query) = slash_command_query(&state.session.prompt.text) else {
        return Vec::new();
    };

    state
        .commands
        .iter()
        .filter(|command| command.name.starts_with(query))
        .map(|command| format!("/{:<17}{}", command.name, command.summary))
        .collect()
}

fn slash_command_query(text: &str) -> Option<&str> {
    let text = text.trim_start();
    if !text.starts_with('/') || text.contains(char::is_whitespace) {
        return None;
    }

    Some(text.strip_prefix('/').unwrap_or_default())
}

fn working_line(state: &State) -> Option<String> {
    match &state.status {
        Status::Running { .. } | Status::Cancelling => Some(format!(
            "{} Working (CTRL + C to stop)",
            state.spinner.current_frame()
        )),
        Status::Idle | Status::Failed { .. } => None,
    }
}

fn worked_summary_text(duration: &str, turn_tokens: Option<u64>) -> String {
    format!(
        "Worked for {duration} · total {} tokens",
        turn_tokens.unwrap_or(0)
    )
}

fn compact_token_count(tokens: u64) -> String {
    if tokens < 1_000 {
        return tokens.to_string();
    }

    format!("{}k", tokens / 1_000)
}

/// Formats a working directory using the current user's home directory when available.
pub fn format_directory(directory: &Path) -> String {
    format_directory_with_home(directory, home_dir().as_deref())
}

/// Formats a directory with an injected home path for deterministic tests.
pub fn format_directory_with_home(directory: &Path, home: Option<&Path>) -> String {
    let Some(home) = home else {
        return directory.display().to_string();
    };
    if directory == home {
        return "~".to_owned();
    }
    let Ok(relative) = directory.strip_prefix(home) else {
        return directory.display().to_string();
    };
    if relative.as_os_str().is_empty() {
        return "~".to_owned();
    }

    format!("~{}{}", MAIN_SEPARATOR, relative.display())
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .or_else(|| {
            let drive = std::env::var_os("HOMEDRIVE")?;
            let path = std::env::var_os("HOMEPATH")?;
            let mut home = drive;
            home.push(path);
            Some(PathBuf::from(home)).filter(|path| !path.as_os_str().is_empty())
        })
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|path| !path.as_os_str().is_empty())
        })
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
