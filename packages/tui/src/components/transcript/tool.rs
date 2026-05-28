use super::content::{
    display_line_render_line, styled_visible_lines, transcript_item_frame, TranscriptRowWrap,
};
use super::TranscriptRenderContext;
use crate::render::{RenderLine, RenderStyle};
use crate::transcript::{ToolCallItem, ToolDisplay};
use iocraft::prelude::*;

/// Renders a tool-call transcript item.
#[component]
pub fn Tool(props: &ToolProps) -> impl Into<AnyElement<'static>> {
    let rows = tool_render_lines_from_fields(
        &props.name,
        props.arguments_preview.as_deref(),
        props.output_preview.as_deref(),
        props.display.as_ref(),
    );

    transcript_item_frame(
        props.source_id.clone(),
        rows,
        props.context.clone(),
        TranscriptRowWrap::NoWrap,
    )
}

/// Props for the tool component.
#[derive(Props)]
pub struct ToolProps {
    pub source_id: String,
    pub name: String,
    pub arguments_preview: Option<String>,
    pub output_preview: Option<String>,
    pub display: Option<ToolDisplay>,
    pub context: TranscriptRenderContext,
}

/// Formats a tool-call transcript item as original-shaped semantic rows.
pub(super) fn tool_render_lines(tool: &ToolCallItem) -> Vec<RenderLine> {
    tool_render_lines_from_fields(
        &tool.name,
        tool.arguments_preview.as_deref(),
        tool.output_preview.as_deref(),
        tool.display.as_ref(),
    )
}

fn tool_render_lines_from_fields(
    name: &str,
    arguments_preview: Option<&str>,
    output_preview: Option<&str>,
    display: Option<&ToolDisplay>,
) -> Vec<RenderLine> {
    if let Some(display) = display {
        let mut lines = Vec::new();
        if let Some(call_line) = &display.call_line {
            lines.push(display_line_render_line(call_line));
        }
        lines.extend(display.argument_lines.iter().map(display_line_render_line));
        lines.extend(display.output_lines.iter().map(display_line_render_line));
        return lines;
    }

    let mut call = name.to_owned();
    if let Some(arguments) = arguments_preview {
        if !arguments.trim().is_empty() {
            call.push(' ');
            call.push_str(arguments);
        }
    }

    let mut lines = vec![RenderLine::styled(call, RenderStyle::Tool)];
    lines.extend(styled_visible_lines(
        output_preview.unwrap_or_default(),
        RenderStyle::CommandOutput,
    ));
    lines
}
