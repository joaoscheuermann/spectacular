use crate::metadata::CommandDescriptor;
use crate::state::State;
use iocraft::prelude::*;

/// Renders prompt text plus command suggestions derived from explicit TUI state.
#[component]
pub fn PromptInput<'a>(props: &PromptInputProps<'a>) -> impl Into<AnyElement<'a>> {
    let state = props.state.expect("PromptInput requires state");
    let prompt = prompt_text(&state.session.prompt.text);
    element! {
        View(border_style: BorderStyle::Single, padding: 1, flex_direction: FlexDirection::Column) {
            Text(content: format!("Prompt: {prompt}"))
            Text(content: completions_text(state))
            Text(content: guidance_text(state))
        }
    }
}

/// Props for the prompt input placeholder component.
#[derive(Default, Props)]
pub struct PromptInputProps<'a> {
    pub state: Option<&'a State>,
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
        .map(command_suggestion_text)
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

    format!("Guidance: /{} - {}", command.name, command.summary)
}

/// Returns the active slash-command query before arguments begin.
fn slash_command_query(text: &str) -> Option<&str> {
    let text = text.trim_start();
    if !text.starts_with('/') || text.contains(char::is_whitespace) {
        return None;
    }

    Some(text.strip_prefix('/').unwrap_or_default())
}

/// Formats one command descriptor as visible completion text.
fn command_suggestion_text(command: &CommandDescriptor) -> String {
    format!("/{} - {}", command.name, command.summary)
}
