use crate::action::ChatTuiAction;
use crate::ids::TranscriptItemId;
use crate::reducer::lookup::{find_command_index, find_tool_call_index};
use crate::reducer::transcript::append_transcript_item;
use crate::state::State;
use crate::transcript::{
    CommandDisplay, CommandDisplayStatus, CommandItem, CommandStatus, DisplayLine, ToolCallItem,
    ToolDisplay, ToolDisplayStatus, ToolStatus, TranscriptItemContent,
};

pub(super) fn reduce(state: &mut State, action: ChatTuiAction) {
    match action {
        ChatTuiAction::ToolCallStarted {
            id,
            tool_call_id,
            name,
            arguments,
        } => {
            let arguments_preview = optional_preview(arguments);
            append_transcript_item(
                state,
                id,
                TranscriptItemContent::ToolCall(ToolCallItem::running(
                    tool_call_id,
                    name,
                    arguments_preview,
                )),
            );
        }
        ChatTuiAction::ToolCallDelta { tool_call_id, text } => {
            append_tool_delta(state, &tool_call_id, &text);
        }
        ChatTuiAction::ToolCallFinished {
            tool_call_id,
            name,
            output,
        } => {
            finish_tool_call(state, &tool_call_id, name, output);
        }
        ChatTuiAction::ToolCallFailed {
            tool_call_id,
            error,
        } => {
            fail_tool_call(state, &tool_call_id, error);
        }
        ChatTuiAction::ToolDisplayStarted {
            id,
            tool_call_id,
            name,
            call_line,
            argument_lines,
        } => {
            append_display_tool_call(state, id, tool_call_id, name, call_line, argument_lines);
        }
        ChatTuiAction::ToolDisplayFinished {
            tool_call_id,
            status,
            output_lines,
        } => {
            finish_display_tool_call(state, &tool_call_id, status, output_lines);
        }
        ChatTuiAction::CommandStarted {
            id,
            command_id,
            command,
        } => {
            append_transcript_item(
                state,
                id,
                TranscriptItemContent::Command(CommandItem::running(command_id, command)),
            );
        }
        ChatTuiAction::CommandOutput { command_id, text } => {
            append_command_output(state, &command_id, &text);
        }
        ChatTuiAction::CommandFinished {
            command_id,
            exit_code,
        } => {
            finish_command(state, &command_id, exit_code);
        }
        ChatTuiAction::CommandDisplayStarted {
            id,
            command_id,
            command_line,
        } => {
            append_display_command(state, id, command_id, command_line);
        }
        ChatTuiAction::CommandDisplayOutput { command_id, chunk } => {
            append_display_command_output(state, &command_id, chunk.line);
        }
        ChatTuiAction::CommandDisplayFinished {
            command_id,
            status,
            exit_code,
            summary_line,
        } => {
            finish_display_command(state, &command_id, status, exit_code, summary_line);
        }
        _ => unreachable!("display reducer received non-display action"),
    }
}

/// Appends a display-ready tool transcript item supplied by the runtime adapter.
pub(crate) fn append_display_tool_call(
    state: &mut State,
    id: TranscriptItemId,
    tool_call_id: String,
    name: String,
    call_line: DisplayLine,
    argument_lines: Vec<DisplayLine>,
) {
    if let Some(index) = find_tool_call_index(state, &tool_call_id) {
        let TranscriptItemContent::ToolCall(tool_call) =
            &mut state.session.transcript[index].content
        else {
            return;
        };
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
    let Some(index) = find_tool_call_index(state, tool_call_id) else {
        return;
    };
    let TranscriptItemContent::ToolCall(tool_call) = &mut state.session.transcript[index].content
    else {
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
    if let Some(index) = find_command_index(state, &command_id) {
        let TranscriptItemContent::Command(command) = &mut state.session.transcript[index].content
        else {
            return;
        };
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
    let Some(index) = find_command_index(state, command_id) else {
        return;
    };
    let TranscriptItemContent::Command(command) = &mut state.session.transcript[index].content
    else {
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
    let Some(index) = find_command_index(state, command_id) else {
        return;
    };
    let TranscriptItemContent::Command(command) = &mut state.session.transcript[index].content
    else {
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

/// Appends incremental output preview text to a tool call by lifecycle ID.
fn append_tool_delta(state: &mut State, tool_call_id: &str, text: &str) {
    let Some(index) = find_tool_call_index(state, tool_call_id) else {
        return;
    };
    let TranscriptItemContent::ToolCall(tool_call) = &mut state.session.transcript[index].content
    else {
        return;
    };

    let output = tool_call.output_preview.get_or_insert_with(String::new);
    output.push_str(text);
}

/// Marks a tool call finished while preserving its start-time identity metadata.
fn finish_tool_call(state: &mut State, tool_call_id: &str, _name: String, output: String) {
    let Some(index) = find_tool_call_index(state, tool_call_id) else {
        return;
    };
    let TranscriptItemContent::ToolCall(tool_call) = &mut state.session.transcript[index].content
    else {
        return;
    };

    tool_call.status = ToolStatus::Finished;
    tool_call.output_preview = Some(output);
}

/// Marks a tool call failed and appends the error to its output preview.
fn fail_tool_call(state: &mut State, tool_call_id: &str, error: String) {
    let Some(index) = find_tool_call_index(state, tool_call_id) else {
        return;
    };
    let TranscriptItemContent::ToolCall(tool_call) = &mut state.session.transcript[index].content
    else {
        return;
    };

    tool_call.status = ToolStatus::Failed;
    let output = tool_call.output_preview.get_or_insert_with(String::new);
    output.push_str(&error);
}

/// Appends command output to the matching command transcript item.
fn append_command_output(state: &mut State, command_id: &str, text: &str) {
    let Some(index) = find_command_index(state, command_id) else {
        return;
    };
    let TranscriptItemContent::Command(command) = &mut state.session.transcript[index].content
    else {
        return;
    };

    command.output.push_str(text);
}

/// Marks a command complete and records its exit code.
fn finish_command(state: &mut State, command_id: &str, exit_code: Option<i32>) {
    let Some(index) = find_command_index(state, command_id) else {
        return;
    };
    let TranscriptItemContent::Command(command) = &mut state.session.transcript[index].content
    else {
        return;
    };

    command.status = match exit_code {
        Some(0) | None => CommandStatus::Finished,
        Some(_) => CommandStatus::Failed,
    };
    command.exit_code = exit_code;
}

/// Returns a non-empty preview for optional argument storage.
fn optional_preview(value: String) -> Option<String> {
    if value.is_empty() {
        return None;
    }

    Some(value)
}
