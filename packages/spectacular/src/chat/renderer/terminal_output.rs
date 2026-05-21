use super::footer::{format_user_prompt_footer, UserPromptFooterView};
use super::style::{command_output_style, diff_added_style, diff_removed_style, paint};
use crate::chat::display::{styled_tool_output_lines, ToolCallView, ToolOutputLineStyle};
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

/// Reports whether assistant content contains non-whitespace visible text.
pub(crate) fn has_visible_assistant_text(content: &str) -> bool {
    !content.trim().is_empty()
}

/// Formats prompt footer data with styles owned by each footer segment.
pub(crate) fn format_prompt_footer(footer: &ChatPromptFooterModel) -> String {
    let view = UserPromptFooterView::from_model(footer);
    format_user_prompt_footer(&view)
}
