use super::content::{
    display_line_render_line, styled_visible_lines, transcript_item_frame, TranscriptRowWrap,
};
use super::TranscriptRenderContext;
use crate::render::{RenderLine, RenderStyle};
use crate::transcript::{CommandDisplay, CommandItem, CommandStatus};
use iocraft::prelude::*;

/// Renders a command transcript item.
#[component]
pub fn Command(props: &CommandProps) -> impl Into<AnyElement<'static>> {
    let rows = command_render_lines_from_fields(
        &props.command,
        props.status,
        &props.output,
        props.exit_code,
        props.display.as_ref(),
    );

    transcript_item_frame(
        props.source_id.clone(),
        rows,
        props.context.clone(),
        TranscriptRowWrap::NoWrap,
    )
}

/// Props for the command component.
#[derive(Props)]
pub struct CommandProps {
    pub source_id: String,
    pub command: String,
    pub status: CommandStatus,
    pub output: String,
    pub exit_code: Option<i32>,
    pub display: Option<CommandDisplay>,
    pub context: TranscriptRenderContext,
}

/// Formats a command transcript item as original-shaped semantic rows.
pub(super) fn command_render_lines(command: &CommandItem) -> Vec<RenderLine> {
    command_render_lines_from_fields(
        &command.command,
        command.status,
        &command.output,
        command.exit_code,
        command.display.as_ref(),
    )
}

fn command_render_lines_from_fields(
    command: &str,
    status: CommandStatus,
    output: &str,
    exit_code: Option<i32>,
    display: Option<&CommandDisplay>,
) -> Vec<RenderLine> {
    if let Some(display) = display {
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
        format!("$ {command}"),
        RenderStyle::Command,
    )];
    lines.extend(styled_visible_lines(output, RenderStyle::CommandOutput));
    if status != CommandStatus::Failed {
        return lines;
    }
    if let Some(exit_code) = exit_code {
        lines.push(RenderLine::styled(
            format!("exit: {exit_code}"),
            RenderStyle::Error,
        ));
    }
    lines
}
