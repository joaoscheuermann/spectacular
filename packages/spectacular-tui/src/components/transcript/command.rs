use super::content::{display_line_render_line, styled_visible_lines, visible_text_row_count};
use crate::render::{semantic_iocraft_style, RenderLine, RenderStyle};
use crate::transcript::{
    CommandDisplay, CommandItem, CommandStatus, DisplayLine, DisplayLineStyle, TranscriptItem,
    TranscriptItemContent,
};
use iocraft::prelude::*;

/// Renders a command transcript item.
#[component]
pub fn Command(props: &CommandProps) -> impl Into<AnyElement<'static>> {
    let item = props.item.clone().expect("Command requires item");
    let TranscriptItemContent::Command(command) = item.content else {
        panic!("Command requires command content");
    };

    let rows = command_elements(&command);
    element!(View(flex_direction: FlexDirection::Column, margin_bottom: 1) { #(rows.into_iter()) })
}

/// Props for the command component.
#[derive(Default, Props)]
pub struct CommandProps {
    pub item: Option<TranscriptItem>,
}

/// Renders the command invocation row.
#[component]
fn CommandLine(props: &CommandLineProps) -> impl Into<AnyElement<'static>> {
    let line = props.line.clone().expect("CommandLine requires line");
    let (color, weight) = semantic_iocraft_style(RenderStyle::from(line.style));

    element!(Text(wrap: TextWrap::NoWrap, content: line.text, color, weight))
}

/// Props for a command invocation row.
#[derive(Default, Props)]
struct CommandLineProps {
    line: Option<DisplayLine>,
}

/// Renders one command output row.
#[component]
fn CommandOutputRow(props: &CommandOutputRowProps) -> impl Into<AnyElement<'static>> {
    let line = props.line.clone().expect("CommandOutputRow requires line");
    let (color, weight) = semantic_iocraft_style(RenderStyle::from(line.style));

    element!(Text(wrap: TextWrap::NoWrap, content: line.text, color, weight))
}

/// Props for one command output row.
#[derive(Default, Props)]
struct CommandOutputRowProps {
    line: Option<DisplayLine>,
}

/// Renders the command completion summary row.
#[component]
fn CommandSummary(props: &CommandSummaryProps) -> impl Into<AnyElement<'static>> {
    let line = props.line.clone().expect("CommandSummary requires line");
    let (color, weight) = semantic_iocraft_style(RenderStyle::from(line.style));

    element!(Text(wrap: TextWrap::NoWrap, content: line.text, color, weight))
}

/// Props for a command completion summary row.
#[derive(Default, Props)]
struct CommandSummaryProps {
    line: Option<DisplayLine>,
}

/// Builds command transcript UI rows without flattening through render-line compatibility helpers.
fn command_elements(command: &CommandItem) -> Vec<AnyElement<'static>> {
    if let Some(display) = &command.display {
        return display_command_elements(display);
    }

    legacy_command_elements(command)
}

/// Builds adapter-provided command display rows.
fn display_command_elements(display: &CommandDisplay) -> Vec<AnyElement<'static>> {
    let mut rows = Vec::new();
    if let Some(command_line) = &display.command_line {
        rows.push(
            element!(CommandLine(key: "command-line".to_owned(), line: command_line.clone()))
                .into_any(),
        );
    }

    rows.extend(
        display
            .output_lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                element!(CommandOutputRow(key: format!("output-{index}"), line: line.clone()))
                    .into_any()
            }),
    );

    if let Some(summary_line) = &display.summary_line {
        rows.push(
            element!(CommandSummary(key: "summary-line".to_owned(), line: summary_line.clone()))
                .into_any(),
        );
    }

    rows
}

/// Builds compatibility rows for older command transcript snapshots.
fn legacy_command_elements(command: &CommandItem) -> Vec<AnyElement<'static>> {
    let mut rows = vec![element!(CommandLine(
        key: "command-line".to_owned(),
        line: DisplayLine::new(format!("$ {}", command.command), DisplayLineStyle::Command)
    ))
    .into_any()];

    rows.extend(command.output.lines().enumerate().map(|(index, line)| {
        element!(CommandOutputRow(
            key: format!("output-{index}"),
            line: DisplayLine::new(line, DisplayLineStyle::CommandOutput)
        ))
        .into_any()
    }));

    if command.status == CommandStatus::Failed {
        if let Some(exit_code) = command.exit_code {
            rows.push(
                element!(CommandSummary(
                    key: "summary-line".to_owned(),
                    line: DisplayLine::new(format!("exit: {exit_code}"), DisplayLineStyle::Error)
                ))
                .into_any(),
            );
        }
    }

    rows
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
