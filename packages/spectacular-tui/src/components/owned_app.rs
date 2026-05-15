use crate::components::status_line::usage_text;
use crate::state::State;
use crate::status::Activity;
use crate::transcript::{CommandStatus, ToolStatus, TranscriptItem, TranscriptItemContent};
use crate::transcript_window::visible_transcript_range;
use iocraft::prelude::*;

/// Composes the full IOCraft application layout from owned state for runtime rendering.
#[component]
pub fn AppState(props: &AppStateProps) -> impl Into<AnyElement<'static>> {
    let state = props.state.clone().expect("AppState requires state");
    element! {
        View(flex_direction: FlexDirection::Column, width: 100pct, height: 100pct) {
            View(
                border_style: BorderStyle::Single,
                border_edges: Edges::Bottom,
                padding_bottom: 1,
                flex_direction: FlexDirection::Column,
            ) {
                Text(content: "Spectacular")
                Text(content: format!("model: {}", state.display.model_label))
                Text(content: format!("reasoning: {}", state.display.reasoning_label))
                Text(content: format!("directory: {}", state.display.current_directory))
                Text(content: format!("session: {}", state.display.session_label))
            }
            View(flex_direction: FlexDirection::Column, flex_grow: 1.0, padding_top: 1, padding_bottom: 1) {
                Text(content: "Transcript")
                ScrollView(auto_scroll: true, keyboard_scroll: false, scrollbar: false) {
                    #(transcript_rows(&state))
                }
            }
            View(border_style: BorderStyle::Single, border_edges: Edges::Top, padding_top: 1) {
                Text(content: status_text(&state))
            }
            View(border_style: BorderStyle::Single, padding: 1, flex_direction: FlexDirection::Column) {
                Text(content: format!("Prompt: {}", prompt_text(&state.session.prompt.text)))
                Text(content: completions_text(&state))
                Text(content: guidance_text(&state))
            }
            View(border_style: BorderStyle::Single, border_edges: Edges::Top, padding_top: 1) {
                Text(content: footer_text(&state))
            }
        }
    }
}

/// Props for the owned-state root application component.
#[derive(Default, Props)]
pub struct AppStateProps {
    pub state: Option<State>,
}

/// Converts transcript state into IOCraft text row elements with owned content.
fn transcript_rows(state: &State) -> Vec<AnyElement<'static>> {
    if state.session.transcript.is_empty() {
        return vec![element!(Text(content: "No transcript items yet".to_string())).into()];
    }
    let range = visible_transcript_range(state.session.transcript.len(), &state.scroll);
    state.session.transcript[range]
        .iter()
        .map(|item| element!(Text(content: transcript_item_text(item))).into())
        .collect()
}

/// Formats one semantic transcript item for the runtime IOCraft app.
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

/// Formats the status line content without performing side effects.
fn status_text(state: &State) -> String {
    let usage = usage_text(state.session.usage.or(state.display.usage));
    match &state.status {
        crate::status::Status::Idle => format!("Status: idle | tokens: {usage}"),
        crate::status::Status::Cancelling => format!(
            "Status: cancelling | spinner: {} | tokens: {usage}",
            state.spinner.current_frame()
        ),
        crate::status::Status::Failed { message } => {
            format!("Status: failed | {message} | tokens: {usage}")
        }
        crate::status::Status::Running { activity, .. } => format!(
            "Status: running | spinner: {} | activity: {} | tokens: {usage}",
            state.spinner.current_frame(),
            activity_text(activity)
        ),
    }
}

/// Formats the current activity label shown while running.
fn activity_text(activity: &Activity) -> String {
    match activity {
        Activity::WaitingForModel => "waiting for model".to_string(),
        Activity::StreamingAssistant { .. } => "streaming assistant".to_string(),
        Activity::StreamingReasoning { .. } => "streaming reasoning".to_string(),
        Activity::RunningTool { name, .. } => format!("running tool {name}"),
        Activity::RunningCommand { command_id, .. } => format!("running command {command_id}"),
        Activity::Retrying { attempt } => format!("retrying attempt {attempt}"),
    }
}

/// Returns user prompt text or an empty placeholder label.
fn prompt_text(text: &str) -> &str {
    if text.is_empty() {
        return "<empty>";
    }
    text
}

/// Formats slash-command completion suggestions from state-owned command descriptors.
fn completions_text(state: &State) -> String {
    let Some(query) = slash_command_query(&state.session.prompt.text) else {
        return "Completions: reserved".to_string();
    };

    let suggestions: Vec<String> = state
        .commands
        .iter()
        .filter(|command| command.name.starts_with(query))
        .map(|command| format!("/{} - {}", command.name, command.summary))
        .collect();
    if suggestions.is_empty() {
        return "Completions: none".to_string();
    }

    format!("Completions: {}", suggestions.join(", "))
}

/// Formats command guidance from state-owned commands and the current prompt.
fn guidance_text(state: &State) -> String {
    let text = state.session.prompt.text.trim_end();
    if !text.starts_with('/') {
        return "Guidance: reserved".to_string();
    }

    let command_name = text
        .strip_prefix('/')
        .unwrap_or_default()
        .split_whitespace()
        .next()
        .unwrap_or_default();
    let Some(command) = state
        .commands
        .iter()
        .find(|command| command.name == command_name)
    else {
        return "Guidance: type a slash command or press Enter to submit".to_string();
    };

    let mut guidance = format!("Guidance: /{} - {}", command.name, command.summary);
    if !command.usage.trim().is_empty() {
        guidance.push_str(&format!(" | Usage: {}", command.usage));
    }
    guidance
}

/// Returns the active slash-command query before arguments begin.
fn slash_command_query(text: &str) -> Option<&str> {
    let text = text.trim_start();
    if !text.starts_with('/') || text.contains(char::is_whitespace) {
        return None;
    }

    Some(text.strip_prefix('/').unwrap_or_default())
}

/// Formats footer metadata from display/session state without external lookups.
fn footer_text(state: &State) -> String {
    let usage = usage_text(state.session.usage.or(state.display.usage));
    format!(
        "cwd: {} | provider/model: {}/{} | reasoning: {} | context: {}",
        state.display.current_directory,
        state.display.provider_label,
        state.display.model_label,
        state.display.reasoning_label,
        usage,
    )
}
