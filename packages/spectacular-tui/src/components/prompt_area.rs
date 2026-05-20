use crate::components::transcript_content::render_line_element;
use crate::metadata::CommandDescriptor;
use crate::render_model::{RenderLine, RenderSpan, RenderStyle};
use crate::state::State;
use iocraft::prelude::*;

/// Renders the active prompt rows and contextual suggestions.
#[component]
pub fn PromptArea(props: &PromptAreaProps) -> impl Into<AnyElement<'static>> {
    let state = props.state.clone().expect("PromptArea requires state");
    let lines = prompt_render_lines(&state);

    element!(View(width: 100pct) {
        #(lines.into_iter().map(render_line_element))
    })
}

/// Formats the active prompt with the prompt marker and multiline continuation.
pub fn prompt_render_lines(state: &State) -> Vec<RenderLine> {
    let mut lines = active_prompt_text_render_lines(state);
    lines.extend(slash_suggestion_render_lines(state));
    lines.extend(slash_usage_render_lines(state));
    lines
}

/// Formats the active prompt as plain visible text for compatibility tests.
pub fn prompt_lines(state: &State) -> Vec<String> {
    crate::components::plain_lines(prompt_render_lines(state))
}

/// Formats the active prompt with semantic spans for selected text ranges.
fn active_prompt_text_render_lines(state: &State) -> Vec<RenderLine> {
    let text = &state.session.prompt.text;
    let rows: Vec<&str> = text.lines().collect();
    if rows.is_empty() {
        return vec![RenderLine::styled("> ", RenderStyle::User)];
    }

    let mut offset = 0;
    rows.into_iter()
        .enumerate()
        .map(|(index, line)| {
            let marker = if index == 0 { "> " } else { "  " };
            let line_start = offset;
            let line_end = line_start + line.len();
            offset = line_end + 1;
            selected_prompt_render_line(
                marker,
                line,
                line_start,
                line_end,
                state.session.prompt.selection_range(),
            )
        })
        .collect()
}

/// Formats one prompt row, highlighting the intersection with active selection.
fn selected_prompt_render_line(
    marker: &str,
    line: &str,
    line_start: usize,
    line_end: usize,
    selection: Option<std::ops::Range<usize>>,
) -> RenderLine {
    let Some(selection) = selection else {
        return RenderLine::styled(format!("{marker}{line}"), RenderStyle::User);
    };

    let start = selection.start.max(line_start);
    let end = selection.end.min(line_end);
    if start >= end {
        return RenderLine::styled(format!("{marker}{line}"), RenderStyle::User);
    }

    let local_start = start - line_start;
    let local_end = end - line_start;
    let mut spans = vec![RenderSpan::new(marker, RenderStyle::User)];
    spans.push(RenderSpan::new(&line[..local_start], RenderStyle::User));
    spans.push(RenderSpan::new(
        &line[local_start..local_end],
        RenderStyle::Selection,
    ));
    spans.push(RenderSpan::new(&line[local_end..], RenderStyle::User));
    RenderLine::from_spans(spans)
}

/// Formats slash command suggestions under the active prompt.
fn slash_suggestion_render_lines(state: &State) -> Vec<RenderLine> {
    slash_suggestions(state)
        .into_iter()
        .enumerate()
        .map(|(index, command)| {
            slash_suggestion_render_line(command, index, state.session.prompt.selected_completion)
        })
        .collect()
}

/// Formats selected slash command usage guidance when available.
fn slash_usage_render_lines(state: &State) -> Vec<RenderLine> {
    let suggestions = slash_suggestions(state);
    let command = suggestions
        .get(state.session.prompt.selected_completion)
        .or_else(|| suggestions.first())
        .copied()
        .or_else(|| active_slash_command(state));
    let Some(command) = command else {
        return Vec::new();
    };
    if command.usage.is_empty() {
        return Vec::new();
    }

    vec![RenderLine::styled(&command.usage, RenderStyle::Dim)]
}

/// Returns display-ready slash command suggestions for the active prompt.
fn slash_suggestions(state: &State) -> Vec<&CommandDescriptor> {
    crate::prompt_state::slash_suggestions(&state.session.prompt, &state.commands)
}

/// Returns the accepted leading slash command when the prompt has command arguments.
fn active_slash_command(state: &State) -> Option<&CommandDescriptor> {
    let text = state.session.prompt.text.trim_start();
    let command_name = text.strip_prefix('/')?.split_whitespace().next()?;
    if !text[command_name.len() + 1..].starts_with(char::is_whitespace) {
        return None;
    }

    state
        .commands
        .iter()
        .find(|command| command.name == command_name)
}

/// Formats one slash suggestion row using original padding and selection styles.
fn slash_suggestion_render_line(
    command: &CommandDescriptor,
    index: usize,
    selected: usize,
) -> RenderLine {
    let style = if index == selected {
        RenderStyle::User
    } else {
        RenderStyle::Dim
    };
    let label = format!("/{}", command.name);
    RenderLine::styled(format!("  {label:<18} {}", command.summary), style)
}

/// Props for the prompt area component.
#[derive(Default, Props)]
pub struct PromptAreaProps {
    pub state: Option<State>,
}
