use crate::render::{iocraft_content_with_selection_colors, RenderLine, RenderSpan, RenderStyle};
use crate::selection::{style_line_for_source, SelectableSource};
use crate::session::SelectionPromptState;
use crate::state::State;
use iocraft::prelude::*;

/// Renders an active command-owned option selection prompt.
#[component]
pub fn SelectionPrompt(props: &SelectionPromptProps) -> impl Into<AnyElement<'static>> {
    let state = props.state.clone().expect("SelectionPrompt requires state");
    let elements = selection_prompt_render_lines(&state)
        .into_iter()
        .enumerate()
        .map(|(index, line)| {
            let line = style_line_for_source(
                &state,
                line,
                SelectableSource::SelectionPrompt { line: index },
            );
            let contents = iocraft_content_with_selection_colors(&line, state.selection_colors);
            element!(MixedText(wrap: TextWrap::NoWrap, contents))
        });

    element!(View(flex_direction: FlexDirection::Column, width: 100pct, margin_bottom: 1) { #(elements) })
}

/// Formats the active selection prompt as semantic render rows.
pub fn selection_prompt_render_lines(state: &State) -> Vec<RenderLine> {
    state
        .selection
        .as_ref()
        .map(selection_lines)
        .unwrap_or_default()
}

/// Props for the active selection prompt component.
#[derive(Default, Props)]
pub struct SelectionPromptProps {
    pub state: Option<State>,
}

/// Formats one selection prompt state using the legacy terminal-flow shape.
fn selection_lines(selection: &SelectionPromptState) -> Vec<RenderLine> {
    let mut lines = Vec::new();
    lines.push(RenderLine::styled(&selection.title, RenderStyle::Title));
    if !selection.description.trim().is_empty() {
        lines.extend(
            selection
                .description
                .lines()
                .map(|line| RenderLine::styled(line, RenderStyle::Text)),
        );
    }
    lines.push(RenderLine::text(""));

    for (index, option) in selection.options.iter().enumerate() {
        lines.push(option_line(index, option, selection.selected == index));
    }

    if selection.allow_custom {
        lines.push(custom_line(selection));
    }

    if selection.allow_comment {
        lines.push(RenderLine::text(""));
        lines.push(comment_line(selection));
    }

    lines
}

/// Formats one predefined option row.
fn option_line(index: usize, option: &str, selected: bool) -> RenderLine {
    let marker = if selected { ">" } else { " " };
    let label = format!("{}. {option}", option_letter(index));
    if selected {
        return RenderLine::from_spans(vec![
            RenderSpan::new(format!("{marker} "), RenderStyle::Text),
            RenderSpan::new(label, RenderStyle::Title),
        ]);
    }

    RenderLine::styled(format!("{marker} {label}"), RenderStyle::Text)
}

/// Formats the optional custom free-text row.
fn custom_line(selection: &SelectionPromptState) -> RenderLine {
    let index = selection.options.len();
    let marker = if selection.is_custom_selected() {
        ">"
    } else {
        " "
    };
    let label = format!("{}. ", option_letter(index));
    let (value, value_style) = if selection.custom_input.is_empty() {
        ("Type your option here".to_owned(), RenderStyle::Dim)
    } else {
        (selection.custom_input.clone(), RenderStyle::Text)
    };

    if selection.is_custom_selected() {
        return RenderLine::from_spans(vec![
            RenderSpan::new(format!("{marker} "), RenderStyle::Text),
            RenderSpan::new(label, RenderStyle::Title),
            RenderSpan::new(value, value_style),
        ]);
    }

    RenderLine::styled(format!("{marker} {label}{value}"), RenderStyle::Dim)
}

/// Formats the optional comment hint or editable comment row.
fn comment_line(selection: &SelectionPromptState) -> RenderLine {
    if !selection.is_options_mode() {
        let (value, value_style) = if selection.comment.is_empty() {
            ("Add a comment".to_owned(), RenderStyle::Dim)
        } else {
            (selection.comment.clone(), RenderStyle::Text)
        };
        return RenderLine::from_spans(vec![
            RenderSpan::new("  comment: ", RenderStyle::Text),
            RenderSpan::new(value, value_style),
        ]);
    }

    RenderLine::styled(
        format!(
            "  Press Tab to add a comment on {}.",
            selection.selected_label()
        ),
        RenderStyle::Dim,
    )
}

/// Converts a zero-based option index into the visible option letter.
fn option_letter(index: usize) -> char {
    char::from(b'a' + (index.min(25) as u8))
}
