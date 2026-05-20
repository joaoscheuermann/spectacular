use crate::components::transcript_content::{
    display_line_render_line, render_lines_elements, styled_visible_lines,
};
use crate::render_model::{RenderLine, RenderStyle};
use crate::transcript::{CommandItem, CommandStatus, TranscriptItem, TranscriptItemContent};
use iocraft::prelude::*;

/// Renders a command transcript item.
#[component]
pub fn Command(props: &CommandProps) -> impl Into<AnyElement<'static>> {
    let item = props.item.clone().expect("Command requires item");
    let TranscriptItemContent::Command(command) = item.content else {
        panic!("Command requires command content");
    };
    let lines = render_lines_elements(command_render_lines(&command));

    element!(View(flex_direction: FlexDirection::Column) { #(lines.into_iter()) })
}

/// Props for the command component.
#[derive(Default, Props)]
pub struct CommandProps {
    pub item: Option<TranscriptItem>,
}

/// Formats a command transcript item as original-shaped semantic rows.
pub fn command_render_lines(command: &CommandItem) -> Vec<RenderLine> {
    if let Some(display) = &command.display {
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
        format!("$ {}", command.command),
        RenderStyle::Command,
    )];
    lines.extend(styled_visible_lines(
        &command.output,
        RenderStyle::CommandOutput,
    ));
    if command.status != CommandStatus::Failed {
        return lines;
    }
    if let Some(exit_code) = command.exit_code {
        lines.push(RenderLine::styled(
            format!("exit: {exit_code}"),
            RenderStyle::Error,
        ));
    }
    lines
}

/// Counts rows for a command item without building output rows.
pub fn command_row_count(command: &CommandItem) -> usize {
    if let Some(display) = &command.display {
        return usize::from(display.command_line.is_some())
            + display.output_lines.len()
            + usize::from(display.summary_line.is_some());
    }

    1 + crate::components::transcript_content::visible_text_row_count(&command.output)
        + usize::from(command.status == CommandStatus::Failed && command.exit_code.is_some())
}
