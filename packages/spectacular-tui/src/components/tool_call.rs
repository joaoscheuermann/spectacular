use crate::components::transcript_content::{
    display_line_render_line, render_lines_elements, styled_visible_lines,
};
use crate::render_model::{RenderLine, RenderStyle};
use crate::transcript::{ToolCallItem, TranscriptItem, TranscriptItemContent};
use iocraft::prelude::*;

/// Renders a tool-call transcript item.
#[component]
pub fn ToolCall(props: &ToolCallProps) -> impl Into<AnyElement<'static>> {
    let item = props.item.clone().expect("ToolCall requires item");
    let TranscriptItemContent::ToolCall(tool) = item.content else {
        panic!("ToolCall requires tool-call content");
    };
    let lines = render_lines_elements(tool_render_lines(&tool));

    element!(View(flex_direction: FlexDirection::Column) { #(lines.into_iter()) })
}

/// Props for the tool-call component.
#[derive(Default, Props)]
pub struct ToolCallProps {
    pub item: Option<TranscriptItem>,
}

/// Formats a tool-call transcript item as original-shaped semantic rows.
pub fn tool_render_lines(tool: &ToolCallItem) -> Vec<RenderLine> {
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

/// Counts rows for a tool-call item without building output rows.
pub fn tool_row_count(tool: &ToolCallItem) -> usize {
    if let Some(display) = &tool.display {
        return usize::from(display.call_line.is_some())
            + display.argument_lines.len()
            + display.output_lines.len();
    }

    1 + tool
        .output_preview
        .as_deref()
        .map(crate::components::transcript_content::visible_text_row_count)
        .unwrap_or(0)
}
