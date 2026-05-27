use crate::metadata::CommandDescriptor;
use crate::prompt::grapheme::clamp_boundary;
use crate::session::PromptState;
use std::ops::Range;

/// Returns the display-ready slash command suggestions for the current prompt state.
pub fn slash_suggestions<'a>(
    prompt: &PromptState,
    commands: &'a [CommandDescriptor],
) -> Vec<&'a CommandDescriptor> {
    crate::prompt::command_suggestions(prompt, commands)
        .into_iter()
        .filter(|suggestion| suggestion.kind == crate::prompt::CommandSuggestionKind::Command)
        .filter_map(|suggestion| {
            commands
                .iter()
                .find(|command| command.name == suggestion.replacement)
        })
        .collect()
}

/// Returns the current slash command query when the cursor is in the leading token.
pub fn slash_command_query(text: &str, cursor: usize) -> Option<&str> {
    crate::prompt::composer::command_name_query(text, cursor)
}

/// Normalizes pasted text to the prompt's internal newline representation.
pub fn normalize_paste(value: &str) -> String {
    value.replace("\r\n", "\n").replace('\r', "\n")
}

/// Splits text into logical lines while retaining a trailing empty line.
pub(super) fn split_logical_lines(text: &str) -> Vec<String> {
    let lines: Vec<String> = text.split('\n').map(ToOwned::to_owned).collect();
    if lines.is_empty() {
        return vec![String::new()];
    }

    lines
}

/// Returns leading spaces and tabs from the current logical line.
pub(super) fn current_line_indentation(text: &str, cursor: usize) -> String {
    let start = line_start(text, cursor);
    text[start..]
        .chars()
        .take_while(|character| matches!(character, ' ' | '\t'))
        .collect()
}

/// Returns the byte range for the leading slash token when cursor is inside it.
pub(super) fn slash_token_range(text: &str, cursor: usize) -> Option<Range<usize>> {
    if !text.starts_with('/') {
        return None;
    }

    let token_start = 0;
    let token_end = text[token_start..]
        .find(char::is_whitespace)
        .map(|index| token_start + index)
        .unwrap_or(text.len());
    if cursor < token_start || cursor > token_end {
        return None;
    }

    Some(token_start..token_end)
}

/// Returns the start offset for the line containing cursor.
pub(super) fn line_start(value: &str, cursor: usize) -> usize {
    let cursor = clamp_boundary(value, cursor);
    value[..cursor]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0)
}

/// Returns the end offset for the line containing cursor.
pub(super) fn line_end(value: &str, cursor: usize) -> usize {
    let cursor = clamp_boundary(value, cursor);
    value[cursor..]
        .find('\n')
        .map(|index| cursor + index)
        .unwrap_or(value.len())
}

impl PromptState {
    /// Creates an empty prompt with the cursor at the start of the buffer.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Creates a prompt from text with the cursor placed at the end.
    pub fn from_text(value: impl Into<String>) -> Self {
        let text = normalize_paste(&value.into());
        let cursor = text.len();
        Self {
            lines: split_logical_lines(&text),
            text,
            cursor,
            ..Self::default()
        }
    }

    /// Returns the complete prompt buffer by joining logical lines with line feeds.
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    /// Returns whether the prompt has no user-authored content.
    pub fn is_empty(&self) -> bool {
        self.lines.len() == 1 && self.lines.first().is_none_or(String::is_empty)
    }

    /// Replaces prompt text and keeps legacy public text storage synchronized.
    pub(super) fn set_text(&mut self, text: String) {
        self.lines = split_logical_lines(&text);
        self.text = text;
    }

    /// Replaces prompt logical lines and keeps legacy public text storage synchronized.
    pub(super) fn set_lines(&mut self, lines: Vec<String>) {
        self.lines = if lines.is_empty() {
            vec![String::new()]
        } else {
            lines
        };
        self.text = self.lines.join("\n");
    }
}
