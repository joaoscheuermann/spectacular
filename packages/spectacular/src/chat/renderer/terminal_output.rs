use super::footer::{format_user_prompt_footer, UserPromptFooterView};
use super::style::{command_output_style, diff_added_style, diff_removed_style, paint};
use super::tool::ToolCallView;
use crate::chat::model::ChatPromptFooterModel;

/// Returns the already formatted tool-owned call line.
pub(super) fn format_tool_call_view(view: &ToolCallView) -> String {
    view.line.to_owned()
}

/// Prints tool output with special text styling for diff additions and deletions.
pub(super) fn print_tool_output(output: &str) {
    for line in styled_tool_output_lines(output) {
        println!("{}", paint(line.style.terminal_style(), line.text));
    }
}

/// Formats tool output into lines with original renderer diff semantics.
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

/// One display-ready tool output line with original renderer style classification.
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

impl ToolOutputLineStyle {
    /// Maps a pure tool output style back to terminal styling for the legacy renderer.
    fn terminal_style(&self) -> anstyle::Style {
        match self {
            Self::CommandOutput => command_output_style(),
            Self::DiffAdded => diff_added_style(),
            Self::DiffRemoved => diff_removed_style(),
        }
    }
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

/// Reports whether assistant content contains non-whitespace visible text.
pub(crate) fn has_visible_assistant_text(content: &str) -> bool {
    !content.trim().is_empty()
}

/// Formats prompt footer data with styles owned by each footer segment.
pub(crate) fn format_prompt_footer(footer: &ChatPromptFooterModel) -> String {
    let view = UserPromptFooterView::from_model(footer);
    format_user_prompt_footer(&view)
}
