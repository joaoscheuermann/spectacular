use super::key::{is_ctrl_char, is_paste_burst_text_key, line_break_char_key, should_insert_char};
use super::{action_effects, EventEffect};
use crate::action::ChatTuiAction;
use crate::runtime::{ClipboardService, PasteBurst};
use crate::session::SelectionPromptState;
use crate::state::State;
use iocraft::prelude::{KeyCode, KeyEvent, KeyModifiers};
use std::time::Instant;

/// Handles one key event while a modal selection prompt is active.
pub(super) fn key_effects(
    state: &State,
    key: KeyEvent,
    clipboard: Option<&mut dyn ClipboardService>,
    paste_burst: &mut PasteBurst,
    now: Instant,
) -> Vec<EventEffect> {
    if is_ctrl_char(&key, 'q') {
        paste_burst.clear();
        return vec![EventEffect::RequestExit];
    }
    if is_ctrl_char(&key, 'c') {
        paste_burst.clear();
        return Vec::new();
    }
    if is_ctrl_char(&key, 'x') {
        paste_burst.clear();
        return Vec::new();
    }

    let Some(selection) = state.selection.as_ref() else {
        return Vec::new();
    };
    if is_ctrl_char(&key, 'v') {
        paste_burst.clear();
        return super::paste::selection_effects(selection, clipboard);
    }
    if line_break_char_key(&key).is_some() {
        paste_burst.note_line_break(now);
        return line_break_char_effect(selection);
    }
    if key.code == KeyCode::Enter
        && key.modifiers.contains(KeyModifiers::CONTROL)
        && paste_burst.should_insert_line_break(now)
    {
        paste_burst.note_line_break(now);
        return line_break_char_effect(selection);
    }
    match key.code {
        KeyCode::Enter if key.modifiers == KeyModifiers::NONE => {
            paste_burst.clear();
            submit_effect(selection)
        }
        KeyCode::Esc => {
            paste_burst.clear();
            escape_effect(selection)
        }
        KeyCode::Tab => {
            paste_burst.clear();
            change_effect(selection, |selection| {
                selection.toggle_comment_mode();
            })
        }
        KeyCode::Up | KeyCode::Char('k') => {
            paste_burst.clear();
            change_effect(selection, SelectionPromptState::select_previous)
        }
        KeyCode::Down | KeyCode::Char('j') => {
            paste_burst.clear();
            change_effect(selection, SelectionPromptState::select_next)
        }
        KeyCode::Left => {
            paste_burst.clear();
            change_effect(selection, SelectionPromptState::move_left)
        }
        KeyCode::Right => {
            paste_burst.clear();
            change_effect(selection, SelectionPromptState::move_right)
        }
        KeyCode::Home => {
            paste_burst.clear();
            change_effect(selection, SelectionPromptState::move_to_start)
        }
        KeyCode::End => {
            paste_burst.clear();
            change_effect(selection, SelectionPromptState::move_to_end)
        }
        KeyCode::Backspace => {
            paste_burst.clear();
            change_effect(selection, SelectionPromptState::backspace)
        }
        KeyCode::Delete => {
            paste_burst.clear();
            change_effect(selection, SelectionPromptState::delete_forward)
        }
        KeyCode::Char(character) if should_insert_char(&key, character) => {
            if is_paste_burst_text_key(&key) {
                paste_burst.note_text_key(now);
            } else {
                paste_burst.clear();
            }
            change_effect(selection, |selection| {
                selection.insert_text(&character.to_string());
            })
        }
        _ => {
            paste_burst.clear();
            Vec::new()
        }
    }
}

/// Applies a local selection prompt edit and returns an action when state changed.
pub(super) fn change_effect<F>(selection: &SelectionPromptState, edit: F) -> Vec<EventEffect>
where
    F: FnOnce(&mut SelectionPromptState),
{
    let mut next = selection.clone();
    edit(&mut next);
    if &next == selection {
        return Vec::new();
    }

    action_effects(ChatTuiAction::SelectionPromptChanged(Some(next)))
}

/// Converts paste-emitted line-break character keys into single-line selection text.
fn line_break_char_effect(selection: &SelectionPromptState) -> Vec<EventEffect> {
    change_effect(selection, |selection| selection.insert_text(" "))
}

/// Handles Escape by leaving comment mode or cancelling the modal selection prompt.
fn escape_effect(selection: &SelectionPromptState) -> Vec<EventEffect> {
    let mut next = selection.clone();
    if next.escape() {
        return action_effects(ChatTuiAction::SelectionPromptCancelled);
    }

    if &next == selection {
        return Vec::new();
    }

    action_effects(ChatTuiAction::SelectionPromptChanged(Some(next)))
}

/// Builds a selection prompt submit action when the selected answer is valid.
fn submit_effect(selection: &SelectionPromptState) -> Vec<EventEffect> {
    let Some(answer) = selection.answer() else {
        return Vec::new();
    };

    action_effects(ChatTuiAction::SelectionPromptSubmitted(answer))
}
