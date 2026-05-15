use crate::chat::renderer::{
    styled_tool_output_lines, ToolCallView, ToolOutputLineStyle, ToolResultView, ToolStatus,
};
use serde_json::Value;
use spectacular_agent::{CommandStatus, ToolStorage};
use spectacular_tui::{
    ChatTuiAction, CommandDisplayChunk, CommandDisplayStatus, DisplayLine, DisplayLineStyle,
    ToolDisplayStatus, TranscriptItemId,
};
use std::collections::BTreeMap;

/// Stores tool-specific display state needed while adapting runtime events.
#[derive(Default)]
pub(crate) struct ToolDisplayAdapter {
    next_tool_id: u64,
    tool_transcript_ids: BTreeMap<String, TranscriptItemId>,
    tool_arguments: BTreeMap<String, Value>,
}

impl ToolDisplayAdapter {
    /// Creates tool display state for a fresh TUI adapter stream.
    pub(crate) fn new() -> Self {
        Self {
            next_tool_id: 1,
            ..Self::default()
        }
    }

    /// Builds display-ready tool-call lifecycle actions with adapter-owned transcript identity.
    pub(crate) fn started_actions(
        &mut self,
        tool_call_id: &str,
        name: &str,
        arguments: &str,
        tools: &ToolStorage,
    ) -> Vec<ChatTuiAction> {
        let id = self.tool_transcript_id(tool_call_id);
        self.remember_tool_arguments(tool_call_id, arguments);

        vec![ChatTuiAction::ToolDisplayStarted {
            id,
            tool_call_id: tool_call_id.to_owned(),
            name: name.to_owned(),
            call_line: DisplayLine::new(
                strip_ansi_codes(&ToolCallView::from_parts(name, arguments, tools).line),
                DisplayLineStyle::Tool,
            ),
            argument_lines: Vec::new(),
        }]
    }

    /// Returns the stable transcript id for a provider tool-call id, allocating it once.
    fn tool_transcript_id(&mut self, tool_call_id: &str) -> TranscriptItemId {
        if let Some(id) = self.tool_transcript_ids.get(tool_call_id) {
            return id.clone();
        }

        let id = TranscriptItemId::new(format!("tool-call-{}", self.next_tool_id));
        self.next_tool_id = self.next_tool_id.saturating_add(1);
        self.tool_transcript_ids
            .insert(tool_call_id.to_owned(), id.clone());
        id
    }

    /// Builds display-ready tool completion actions for known and implicit tool starts.
    pub(crate) fn result_actions(
        &mut self,
        tool_call_id: &str,
        name: &str,
        content: &str,
        tools: &ToolStorage,
    ) -> Vec<ChatTuiAction> {
        let known_tool = self.tool_transcript_ids.remove(tool_call_id).is_some();
        let arguments = self.tool_arguments.remove(tool_call_id);
        let result =
            ToolResultView::from_parts_with_arguments(name, content, tools, arguments.as_ref());
        let mut actions = Vec::new();
        if !known_tool {
            actions.extend(self.started_actions(tool_call_id, name, "", tools));
            self.tool_transcript_ids.remove(tool_call_id);
        }

        actions.push(ChatTuiAction::ToolDisplayFinished {
            tool_call_id: tool_call_id.to_owned(),
            status: tool_display_status(&result),
            output_lines: tool_output_display_lines(&result),
        });
        actions
    }

    /// Caches parsed tool-call arguments for registered result formatters.
    fn remember_tool_arguments(&mut self, tool_call_id: &str, arguments: &str) {
        let Ok(arguments) = serde_json::from_str::<Value>(arguments) else {
            return;
        };

        self.tool_arguments
            .insert(tool_call_id.to_owned(), arguments);
    }
}

/// Builds display-ready command start action for the TUI reducer.
pub(crate) fn command_started_action(command_id: &str, command: &str) -> ChatTuiAction {
    ChatTuiAction::CommandDisplayStarted {
        id: TranscriptItemId::new(format!("command-{command_id}")),
        command_id: command_id.to_owned(),
        command_line: DisplayLine::new(command, DisplayLineStyle::Command),
    }
}

/// Builds display-ready command output action for the TUI reducer.
pub(crate) fn command_output_action(command_id: &str, content: &str) -> ChatTuiAction {
    ChatTuiAction::CommandDisplayOutput {
        command_id: command_id.to_owned(),
        chunk: CommandDisplayChunk::new(format!("• {content}"), DisplayLineStyle::CommandOutput),
    }
}

/// Builds display-ready command finish action for the TUI reducer.
pub(crate) fn command_finished_action(
    command_id: &str,
    status: CommandStatus,
    summary: &str,
) -> ChatTuiAction {
    ChatTuiAction::CommandDisplayFinished {
        command_id: command_id.to_owned(),
        status: command_display_status(status),
        exit_code: command_exit_code(status),
        summary_line: Some(command_summary_line(status, summary)),
    }
}

/// Converts original renderer tool result status into TUI display status.
fn tool_display_status(view: &ToolResultView) -> ToolDisplayStatus {
    match view.status {
        ToolStatus::Done => ToolDisplayStatus::Succeeded,
        ToolStatus::Failed => ToolDisplayStatus::Failed,
    }
}

/// Converts formatted tool output into display-ready rows with renderer-parity styles.
fn tool_output_display_lines(view: &ToolResultView) -> Vec<DisplayLine> {
    let style = match view.status {
        ToolStatus::Done => None,
        ToolStatus::Failed => Some(DisplayLineStyle::Error),
    };

    styled_tool_output_lines(&view.output)
        .into_iter()
        .map(|line| {
            DisplayLine::new(
                strip_ansi_codes(&line.text),
                style.unwrap_or_else(|| tool_output_display_style(line.style)),
            )
        })
        .collect()
}

/// Maps pure tool output styles into serializable display line styles.
fn tool_output_display_style(style: ToolOutputLineStyle) -> DisplayLineStyle {
    match style {
        ToolOutputLineStyle::CommandOutput => DisplayLineStyle::CommandOutput,
        ToolOutputLineStyle::DiffAdded => DisplayLineStyle::DiffAdded,
        ToolOutputLineStyle::DiffRemoved => DisplayLineStyle::DiffRemoved,
    }
}

/// Removes terminal escape sequences before handing display text to the TUI renderer.
fn strip_ansi_codes(value: &str) -> String {
    let mut output = String::new();
    let mut characters = value.chars().peekable();
    while let Some(character) = characters.next() {
        if character != '\u{1b}' {
            output.push(character);
            continue;
        }

        if characters.peek() != Some(&'[') {
            continue;
        }
        characters.next();
        for code_character in characters.by_ref() {
            if code_character.is_ascii_alphabetic() {
                break;
            }
        }
    }

    output
}

/// Converts command lifecycle status into adapter-owned display status.
fn command_display_status(status: CommandStatus) -> CommandDisplayStatus {
    match status {
        CommandStatus::Success => CommandDisplayStatus::Succeeded,
        CommandStatus::Cancelled => CommandDisplayStatus::Cancelled,
        CommandStatus::Failed | CommandStatus::TimedOut | CommandStatus::Error => {
            CommandDisplayStatus::Failed
        }
    }
}

/// Builds the original renderer-parity command completion summary row.
fn command_summary_line(status: CommandStatus, summary: &str) -> DisplayLine {
    DisplayLine::new(summary, command_summary_style(status))
}

/// Maps command completion status to original renderer line styles.
fn command_summary_style(status: CommandStatus) -> DisplayLineStyle {
    match status {
        CommandStatus::Success => DisplayLineStyle::Success,
        CommandStatus::Failed | CommandStatus::Error => DisplayLineStyle::Error,
        CommandStatus::Cancelled | CommandStatus::TimedOut => DisplayLineStyle::Warning,
    }
}

/// Converts command lifecycle status into the reducer's exit-code based command completion.
fn command_exit_code(status: CommandStatus) -> Option<i32> {
    match status {
        CommandStatus::Success => Some(0),
        CommandStatus::Failed
        | CommandStatus::Cancelled
        | CommandStatus::TimedOut
        | CommandStatus::Error => Some(1),
    }
}
