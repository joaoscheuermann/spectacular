use super::content::{
    display_line_render_line, selectable_line, styled_visible_lines, visible_text_row_count,
};
use super::TranscriptRenderContext;
use crate::render::{iocraft_content_with_selection_colors, RenderLine, RenderStyle};
use crate::transcript::{CommandItem, CommandStatus, TranscriptItem, TranscriptItemContent};
use iocraft::prelude::*;

/// Renders a command transcript item.
#[component]
pub fn Command(props: &CommandProps) -> impl Into<AnyElement<'static>> {
    let item = props.item.clone().expect("Command requires item");
    let context = props.context.as_ref();
    let selection_colors = context
        .map(|context| context.selection_colors)
        .unwrap_or_default();
    let item_id = item.id.as_str().to_owned();
    let TranscriptItemContent::Command(command) = item.content else {
        panic!("Command requires command content");
    };

    let rows = command_render_lines(&command)
        .into_iter()
        .enumerate()
        .map(|(index, line)| {
            let line = selectable_line(context, &item_id, index, line);
            let contents = iocraft_content_with_selection_colors(&line, selection_colors);
            element!(MixedText(wrap: TextWrap::NoWrap, contents))
        });
    element!(View(flex_direction: FlexDirection::Column, margin_bottom: 1) { #(rows.into_iter()) })
}

/// Props for the command component.
#[derive(Default, Props)]
pub struct CommandProps {
    pub item: Option<TranscriptItem>,
    pub context: Option<TranscriptRenderContext>,
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

    1 + visible_text_row_count(&command.output)
        + usize::from(command.status == CommandStatus::Failed && command.exit_code.is_some())
}
