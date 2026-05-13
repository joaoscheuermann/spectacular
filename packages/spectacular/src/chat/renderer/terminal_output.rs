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
    let is_diff_output = output
        .lines()
        .next()
        .is_some_and(|line| line.starts_with("Edited "));
    for line in output.lines() {
        println!("{}", format_tool_output_line(line, is_diff_output));
    }
}

/// Applies command-output or diff-row styling to one tool output line.
fn format_tool_output_line(line: &str, is_diff_output: bool) -> String {
    let style = if is_diff_output && is_added_diff_line(line) {
        diff_added_style()
    } else if is_diff_output && is_removed_diff_line(line) {
        diff_removed_style()
    } else {
        command_output_style()
    };

    paint(style, line)
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
