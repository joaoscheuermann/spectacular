use crate::ids::TranscriptItemId;
use crate::reducer::append_transcript_item;
use crate::reducer_lookup::{find_command, find_tool_call};
use crate::state::State;
use crate::transcript::{
    CommandDisplay, CommandDisplayStatus, CommandItem, CommandStatus, DisplayLine, ToolCallItem,
    ToolDisplay, ToolDisplayStatus, ToolStatus, TranscriptItemContent,
};

/// Appends a display-ready tool transcript item supplied by the runtime adapter.
pub(crate) fn append_display_tool_call(
    state: &mut State,
    id: TranscriptItemId,
    tool_call_id: String,
    name: String,
    call_line: DisplayLine,
    argument_lines: Vec<DisplayLine>,
) {
    if let Some(tool_call) = find_tool_call(state, &tool_call_id) {
        tool_call.name = name;
        let display = tool_call.display.get_or_insert_with(ToolDisplay::default);
        display.call_line = Some(call_line);
        display.argument_lines = argument_lines;
        return;
    }

    let mut item = ToolCallItem::running(tool_call_id, name, None);
    item.display = Some(ToolDisplay {
        call_line: Some(call_line),
        argument_lines,
        output_lines: Vec::new(),
    });
    append_transcript_item(state, id, TranscriptItemContent::ToolCall(item));
}

/// Completes a display-ready tool item without inferring rendering semantics in the TUI.
pub(crate) fn finish_display_tool_call(
    state: &mut State,
    tool_call_id: &str,
    status: ToolDisplayStatus,
    output_lines: Vec<DisplayLine>,
) {
    let Some(tool_call) = find_tool_call(state, tool_call_id) else {
        return;
    };

    tool_call.status = tool_status_from_display(status);
    let display = tool_call.display.get_or_insert_with(ToolDisplay::default);
    display.output_lines = output_lines;
}

/// Converts adapter-owned tool completion status into transcript lifecycle status.
fn tool_status_from_display(status: ToolDisplayStatus) -> ToolStatus {
    match status {
        ToolDisplayStatus::Succeeded => ToolStatus::Finished,
        ToolDisplayStatus::Failed | ToolDisplayStatus::Cancelled => ToolStatus::Failed,
    }
}

/// Appends a display-ready command transcript item supplied by the runtime adapter.
pub(crate) fn append_display_command(
    state: &mut State,
    id: TranscriptItemId,
    command_id: String,
    command_line: DisplayLine,
) {
    if let Some(command) = find_command(state, &command_id) {
        command.command = command_line.text.clone();
        let display = command.display.get_or_insert_with(CommandDisplay::default);
        display.command_line = Some(command_line);
        return;
    }

    let mut item = CommandItem::running(command_id, command_line.text.clone());
    item.display = Some(CommandDisplay {
        command_line: Some(command_line),
        output_lines: Vec::new(),
        summary_line: None,
    });
    append_transcript_item(state, id, TranscriptItemContent::Command(item));
}

/// Appends one display-ready command output line to the matching command item.
pub(crate) fn append_display_command_output(
    state: &mut State,
    command_id: &str,
    line: DisplayLine,
) {
    let Some(command) = find_command(state, command_id) else {
        return;
    };

    let display = command.display.get_or_insert_with(CommandDisplay::default);
    display.output_lines.push(line);
}

/// Marks a display-ready command complete and stores adapter-provided summary data.
pub(crate) fn finish_display_command(
    state: &mut State,
    command_id: &str,
    status: CommandDisplayStatus,
    exit_code: Option<i32>,
    summary_line: Option<DisplayLine>,
) {
    let Some(command) = find_command(state, command_id) else {
        return;
    };

    command.status = command_status_from_display(status);
    command.exit_code = exit_code;
    let display = command.display.get_or_insert_with(CommandDisplay::default);
    display.summary_line = summary_line;
}

/// Converts adapter-owned command completion status into transcript lifecycle status.
fn command_status_from_display(status: CommandDisplayStatus) -> CommandStatus {
    match status {
        CommandDisplayStatus::Succeeded => CommandStatus::Finished,
        CommandDisplayStatus::Failed | CommandDisplayStatus::Cancelled => CommandStatus::Failed,
    }
}
