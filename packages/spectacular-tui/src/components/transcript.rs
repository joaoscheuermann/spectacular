use crate::state::State;
use crate::transcript::{CommandStatus, ToolStatus, TranscriptItem, TranscriptItemContent};
use crate::transcript_window::visible_transcript_range;
use iocraft::prelude::*;

/// Renders semantic transcript items in a read-only scroll region.
#[component]
pub fn TranscriptScrollView<'a>(
    props: &TranscriptScrollViewProps<'a>,
) -> impl Into<AnyElement<'a>> {
    let state = props.state.expect("TranscriptScrollView requires state");
    element! {
        View(flex_direction: FlexDirection::Column, flex_grow: 1.0, padding_top: 1, padding_bottom: 1) {
            Text(content: "Transcript")
            ScrollView(auto_scroll: true, keyboard_scroll: false, scrollbar: false) {
                #(transcript_rows(state))
            }
        }
    }
}

/// Props for the transcript scroll view component.
#[derive(Default, Props)]
pub struct TranscriptScrollViewProps<'a> {
    pub state: Option<&'a State>,
}

/// Converts transcript state into IOCraft text row elements.
fn transcript_rows<'a>(state: &'a State) -> Vec<AnyElement<'a>> {
    if state.session.transcript.is_empty() {
        return vec![element!(Text(content: "No transcript items yet".to_string())).into()];
    }
    visible_transcript_items(state)
        .map(|item| element!(Text(content: transcript_item_text(item))).into())
        .collect()
}

/// Returns the transcript item range that should be materialized for the viewport.
fn visible_transcript_items(state: &State) -> impl Iterator<Item = &TranscriptItem> {
    let range = visible_transcript_range(state.session.transcript.len(), &state.scroll);
    state.session.transcript[range].iter()
}

/// Formats one semantic transcript item for the read-only prototype.
fn transcript_item_text(item: &TranscriptItem) -> String {
    match &item.content {
        TranscriptItemContent::UserPrompt(prompt) => format!("You: {}", prompt.text),
        TranscriptItemContent::AssistantMessage(message) => {
            format!("Assistant: {}", message.text)
        }
        TranscriptItemContent::Reasoning(reasoning) => format!("Reasoning: {}", reasoning.text),
        TranscriptItemContent::ToolCall(tool) => tool_text(tool),
        TranscriptItemContent::Command(command) => command_text(command),
        TranscriptItemContent::Error(error) => error_text(&error.message, error.details.as_deref()),
        TranscriptItemContent::Notice(notice) => format!("Notice: {}", notice.message),
    }
}

/// Formats a tool-call transcript item with status and previews.
fn tool_text(tool: &crate::transcript::ToolCallItem) -> String {
    let mut text = format!("Tool: {} [{}]", tool.name, tool_status_text(tool.status));
    if let Some(arguments) = &tool.arguments_preview {
        text.push_str(&format!(" | args: {arguments}"));
    }
    if let Some(output) = &tool.output_preview {
        text.push_str(&format!(" | output: {output}"));
    }
    text
}

/// Formats a command transcript item with status, output, and exit code.
fn command_text(command: &crate::transcript::CommandItem) -> String {
    let mut text = format!(
        "Command: {} [{}]",
        command.command,
        command_status_text(command.status)
    );
    if !command.output.is_empty() {
        text.push_str(&format!(" | output: {}", command.output));
    }
    if let Some(exit_code) = command.exit_code {
        text.push_str(&format!(" | exit: {exit_code}"));
    }
    text
}

/// Formats an error transcript item and optional details.
fn error_text(message: &str, details: Option<&str>) -> String {
    let Some(details) = details else {
        return format!("Error: {message}");
    };
    format!("Error: {message} | {details}")
}

/// Returns the display label for tool status values.
fn tool_status_text(status: ToolStatus) -> &'static str {
    match status {
        ToolStatus::Running => "running",
        ToolStatus::Finished => "finished",
        ToolStatus::Failed => "failed",
    }
}

/// Returns the display label for command status values.
fn command_status_text(status: CommandStatus) -> &'static str {
    match status {
        CommandStatus::Running => "running",
        CommandStatus::Finished => "finished",
        CommandStatus::Failed => "failed",
    }
}
