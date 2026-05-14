use crate::metadata::ContextTokenUsage;
use crate::state::State;
use crate::status::{Activity, Status};
use iocraft::prelude::*;

/// Renders runtime status, current activity, spinner, and token usage from state.
#[component]
pub fn StatusLine<'a>(props: &StatusLineProps<'a>) -> impl Into<AnyElement<'a>> {
    let state = props.state.expect("StatusLine requires state");
    element! {
        View(border_style: BorderStyle::Single, border_edges: Edges::Top, padding_top: 1) {
            Text(content: status_text(state))
        }
    }
}

/// Props for the status line component.
#[derive(Default, Props)]
pub struct StatusLineProps<'a> {
    pub state: Option<&'a State>,
}

/// Formats the status line content without performing side effects.
fn status_text(state: &State) -> String {
    let usage = usage_text(state.session.usage.or(state.display.usage));
    match &state.status {
        Status::Idle => format!("Status: idle | tokens: {usage}"),
        Status::Cancelling => format!(
            "Status: cancelling | spinner: {} | tokens: {usage}",
            state.spinner.current_frame()
        ),
        Status::Failed { message } => format!("Status: failed | {message} | tokens: {usage}"),
        Status::Running { activity, .. } => format!(
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

/// Formats optional context token usage for status/footer display.
pub fn usage_text(usage: Option<ContextTokenUsage>) -> String {
    let Some(usage) = usage else {
        return "unavailable".to_string();
    };
    let Some(window) = usage.context_window_tokens else {
        return usage.input_tokens.to_string();
    };
    format!("{}/{}", usage.input_tokens, window)
}
