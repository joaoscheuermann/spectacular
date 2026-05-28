use ::agent::ToolStorage;
use serde_json::Value;

/// Display model for a tool call line.
pub(crate) struct ToolCallView {
    pub line: String,
}

/// Display model for a tool result block.
pub(crate) struct ToolResultView {
    pub output: String,
    pub status: ToolStatus,
}

/// Display-neutral status for a completed tool result.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ToolStatus {
    Done,
    Failed,
}

/// One display-ready tool output line with style classification.
pub(crate) struct StyledToolOutputLine {
    pub text: String,
    pub style: ToolOutputLineStyle,
}

/// Style classification for pure tool output formatting.
pub(crate) enum ToolOutputLineStyle {
    CommandOutput,
    DiffAdded,
    DiffRemoved,
}

impl ToolCallView {
    /// Builds display data for a tool call, preferring registered tool formatting.
    pub(crate) fn from_parts(name: &str, arguments: &str, tools: &ToolStorage) -> Self {
        let name = if name.trim().is_empty() { "tool" } else { name };
        let parsed_arguments = serde_json::from_str::<Value>(arguments).ok();
        let line = match (tools.get(name), parsed_arguments.as_ref()) {
            (Some(tool), Some(arguments)) => tool.format_call(arguments),
            _ => format_fallback_call(name, arguments),
        };

        Self { line }
    }
}

impl ToolResultView {
    /// Builds display data for a tool result, passing original call arguments when available.
    pub(crate) fn from_parts_with_arguments(
        name: &str,
        content: &str,
        tools: &ToolStorage,
        arguments: Option<&Value>,
    ) -> Self {
        let parsed = serde_json::from_str::<Value>(content).ok();
        let output = match tools.get(name) {
            Some(tool) => tool.format_output_with_input(content, parsed.as_ref(), arguments),
            None => format_json_preview(content),
        };

        Self {
            output,
            status: if result_failed(content, parsed.as_ref()) {
                ToolStatus::Failed
            } else {
                ToolStatus::Done
            },
        }
    }
}

/// Formats tool output into lines with terminal-compatible diff semantics.
pub(crate) fn styled_tool_output_lines(output: &str) -> Vec<StyledToolOutputLine> {
    let is_diff_output = output
        .lines()
        .next()
        .is_some_and(|line| line.starts_with("Edited "));
    output
        .lines()
        .map(|line| StyledToolOutputLine {
            text: line.to_owned(),
            style: tool_output_line_style(line, is_diff_output),
        })
        .collect()
}

/// Formats unregistered or malformed tool calls with generic display parts.
fn format_fallback_call(name: &str, arguments: &str) -> String {
    let preview = format_json_preview(arguments);
    format_tool_call_parts(name, &preview, None)
}

/// Formats compatibility display parts with shared TUI styles.
pub(crate) fn format_tool_call_parts(label: &str, input: &str, metadata: Option<&str>) -> String {
    tui::tool_line(label, input, metadata)
}

/// Reports whether a tool output matches common failure markers.
fn result_failed(content: &str, parsed: Option<&Value>) -> bool {
    if content.trim_start().starts_with("Error:") {
        return true;
    }

    let Some(value) = parsed else {
        return false;
    };

    if value.get("error").is_some() || value.get("error_kind").is_some() {
        return true;
    }

    value
        .get("exit_code")
        .and_then(Value::as_i64)
        .is_some_and(|exit_code| exit_code != 0)
}

/// Applies command-output or diff-row styling to one tool output line.
fn tool_output_line_style(line: &str, is_diff_output: bool) -> ToolOutputLineStyle {
    if is_diff_output && is_added_diff_line(line) {
        return ToolOutputLineStyle::DiffAdded;
    }
    if is_diff_output && is_removed_diff_line(line) {
        return ToolOutputLineStyle::DiffRemoved;
    }

    ToolOutputLineStyle::CommandOutput
}

/// Reports whether a rendered diff line represents an insertion.
fn is_added_diff_line(line: &str) -> bool {
    diff_line_marker(line) == Some('+')
}

/// Reports whether a rendered diff line represents a deletion.
fn is_removed_diff_line(line: &str) -> bool {
    diff_line_marker(line) == Some('-')
}

/// Returns the marker character after a rendered diff line number.
fn diff_line_marker(line: &str) -> Option<char> {
    let mut characters = line.trim_start().chars().peekable();
    let mut saw_digit = false;
    while characters.peek().is_some_and(char::is_ascii_digit) {
        saw_digit = true;
        characters.next();
    }
    if !saw_digit {
        return None;
    }

    if !characters
        .peek()
        .is_some_and(|character| character.is_whitespace())
    {
        return None;
    }
    while characters
        .peek()
        .is_some_and(|character| character.is_whitespace())
    {
        characters.next();
    }

    characters
        .next()
        .filter(|marker| matches!(marker, '+' | '-'))
}

/// Formats raw JSON or plain text into the compact terminal preview used for unknown tools.
fn format_json_preview(value: &str) -> String {
    let parsed = serde_json::from_str::<Value>(value);
    let value = match parsed {
        Ok(Value::Object(map)) => map
            .into_iter()
            .map(|(key, value)| format!("{key}: {}", compact_value(&value)))
            .collect::<Vec<_>>()
            .join(", "),
        Ok(value) => compact_value(&value),
        Err(_) => value.to_owned(),
    };

    const LIMIT: usize = 180;
    if value.chars().count() <= LIMIT {
        return value;
    }

    value.chars().take(LIMIT).collect::<String>() + "..."
}

/// Converts one JSON value into the concise text used inside preview fields.
fn compact_value(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        _ => value.to_string(),
    }
}
