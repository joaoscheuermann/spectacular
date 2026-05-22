use super::command;
use super::paste;
use super::selection;
use super::{action_effects, is_key_release, prompt_change_effect, EventEffect};
use crate::action::ChatTuiAction;
use crate::runtime::{ClipboardService, PasteBurst};
use crate::session::PromptState;
use crate::state::State;
use iocraft::prelude::{KeyCode, KeyEvent, KeyModifiers};
use std::time::Instant;

/// Returns true when the key path may need native clipboard access.
pub(super) fn needs_system_clipboard(state: &State, key: &KeyEvent) -> bool {
    if is_key_release(key.kind) {
        return false;
    }
    if state.selection.is_some() {
        return is_ctrl_char(key, 'v');
    }
    if is_ctrl_char(key, 'v') {
        return true;
    }
    if is_ctrl_char(key, 'c') || is_ctrl_char(key, 'x') {
        return state.session.prompt.selection_range().is_some();
    }

    false
}

/// Converts one key event into a reducer action or shell-level exit request.
pub(super) fn effects(
    state: &State,
    key: KeyEvent,
    clipboard: Option<&mut dyn ClipboardService>,
    paste_burst: &mut PasteBurst,
    now: Instant,
) -> Vec<EventEffect> {
    if is_key_release(key.kind) {
        return Vec::new();
    }

    if state.selection.is_some() {
        return selection::key_effects(state, key, clipboard, paste_burst, now);
    }

    if is_ctrl_char(&key, 'q') {
        paste_burst.clear();
        return vec![EventEffect::RequestExit];
    }

    if is_ctrl_char(&key, 'c') {
        paste_burst.clear();
        return paste::copy_effects(state, clipboard);
    }
    if is_ctrl_char(&key, 'x') {
        paste_burst.clear();
        return paste::cut_effects(state, clipboard);
    }
    if is_ctrl_char(&key, 'v') {
        paste_burst.clear();
        return paste::effects(state, clipboard);
    }
    if is_ctrl_char(&key, 'z') {
        paste_burst.clear();
        return prompt_change_effect(state, PromptState::undo);
    }
    if is_ctrl_char(&key, 'r') {
        paste_burst.clear();
        return prompt_change_effect(state, PromptState::redo);
    }
    if is_ctrl_char(&key, 'a') {
        paste_burst.clear();
        return prompt_change_effect(state, PromptState::select_all);
    }
    if is_ctrl_char(&key, 'u') {
        paste_burst.clear();
        return prompt_change_effect(state, PromptState::kill_to_line_start);
    }
    if is_ctrl_char(&key, 'k') {
        paste_burst.clear();
        return prompt_change_effect(state, PromptState::kill_to_line_end);
    }
    if is_ctrl_char(&key, 'y') {
        paste_burst.clear();
        return prompt_change_effect(state, PromptState::yank);
    }
    if let Some(line_break) = line_break_char_key(&key) {
        paste_burst.note_line_break(now);
        return prompt_change_effect(state, |prompt| prompt.insert_text(line_break));
    }
    if is_submit_key(&key) {
        if paste_burst.should_insert_line_break(now) {
            paste_burst.note_line_break(now);
            return prompt_change_effect(state, |prompt| prompt.insert_text("\n"));
        }
        paste_burst.clear();
        return command::submit_or_compose_effects(state);
    }
    if is_line_break_key(&key) {
        if paste_burst.should_insert_line_break(now) {
            paste_burst.note_line_break(now);
            return prompt_change_effect(state, |prompt| prompt.insert_text("\n"));
        }
        paste_burst.clear();
        if crate::prompt::has_command_context(&state.session.prompt) {
            return command::submit_or_compose_effects(state);
        }
        return prompt_change_effect(state, PromptState::insert_newline);
    }

    let selecting = is_shift_arrow(&key) || key.modifiers.contains(KeyModifiers::SHIFT);
    let by_word = key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
    match key.code {
        KeyCode::Tab => {
            paste_burst.clear();
            command::tab_effects(state)
        }
        KeyCode::Esc => {
            paste_burst.clear();
            escape_effects(state)
        }
        KeyCode::Char(' ') => {
            paste_burst.note_text_key(now);
            command::space_effects(state)
        }
        KeyCode::Char(character) if should_insert_char(&key, character) => {
            if is_paste_burst_text_key(&key) {
                paste_burst.note_text_key(now);
            } else {
                paste_burst.clear();
            }
            prompt_change_effect(state, |prompt| {
                prompt.insert_character(character);
            })
        }
        KeyCode::Backspace if by_word => {
            paste_burst.clear();
            prompt_change_effect(state, PromptState::delete_previous_word)
        }
        KeyCode::Delete if by_word => {
            paste_burst.clear();
            prompt_change_effect(state, PromptState::delete_next_word)
        }
        KeyCode::Backspace => {
            paste_burst.clear();
            prompt_change_effect(state, PromptState::backspace)
        }
        KeyCode::Delete => {
            paste_burst.clear();
            prompt_change_effect(state, PromptState::delete_forward)
        }
        KeyCode::Left if by_word => {
            paste_burst.clear();
            prompt_change_effect(state, |prompt| prompt.move_word_left(selecting))
        }
        KeyCode::Right if by_word => {
            paste_burst.clear();
            prompt_change_effect(state, |prompt| prompt.move_word_right(selecting))
        }
        KeyCode::Left => {
            paste_burst.clear();
            prompt_change_effect(state, |prompt| prompt.move_left(selecting))
        }
        KeyCode::Right => {
            paste_burst.clear();
            prompt_change_effect(state, |prompt| prompt.move_right(selecting))
        }
        KeyCode::Up => {
            paste_burst.clear();
            command::prompt_up_effects(state, selecting)
        }
        KeyCode::Down => {
            paste_burst.clear();
            command::prompt_down_effects(state, selecting)
        }
        KeyCode::Home if key.modifiers.contains(KeyModifiers::SHIFT) => {
            paste_burst.clear();
            prompt_change_effect(state, |prompt| prompt.move_to_start(selecting))
        }
        KeyCode::End if key.modifiers.contains(KeyModifiers::SHIFT) => {
            paste_burst.clear();
            prompt_change_effect(state, |prompt| prompt.move_to_end(selecting))
        }
        KeyCode::Home if key.modifiers.contains(KeyModifiers::CONTROL) => {
            paste_burst.clear();
            prompt_change_effect(state, |prompt| prompt.move_to_start(selecting))
        }
        KeyCode::End if key.modifiers.contains(KeyModifiers::CONTROL) => {
            paste_burst.clear();
            prompt_change_effect(state, |prompt| prompt.move_to_end(selecting))
        }
        KeyCode::Home => {
            paste_burst.clear();
            prompt_change_effect(state, |prompt| prompt.move_line_start(selecting))
        }
        KeyCode::End => {
            paste_burst.clear();
            prompt_change_effect(state, |prompt| prompt.move_line_end(selecting))
        }
        _ => {
            paste_burst.clear();
            Vec::new()
        }
    }
}

/// Builds Escape behavior from current status without mutating state directly.
fn escape_effects(state: &State) -> Vec<EventEffect> {
    let dismissed = prompt_change_effect(state, |prompt| {
        crate::prompt::dismiss_command_suggestions(prompt, &state.commands);
    });
    if !dismissed.is_empty() {
        return dismissed;
    }

    if state.status.is_cancellable() {
        return action_effects(ChatTuiAction::CancelRun);
    }

    if state.session.prompt.is_empty() {
        return vec![EventEffect::RequestExit];
    }

    prompt_change_effect(state, PromptState::escape)
}

/// Returns true for printable key events without control or alt chords.
pub(super) fn should_insert_char(key: &KeyEvent, character: char) -> bool {
    !character.is_control()
        && !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
}

/// Returns true for text keys that can be part of an unbracketed paste burst.
pub(super) fn is_paste_burst_text_key(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char(character) if !character.is_control())
        && !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
}

/// Returns true when a key event should submit the active prompt.
fn is_submit_key(key: &KeyEvent) -> bool {
    key.code == KeyCode::Enter && key.modifiers.contains(KeyModifiers::CONTROL)
}

/// Returns true when a key event should insert a multiline prompt line break.
fn is_line_break_key(key: &KeyEvent) -> bool {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return false;
    }

    key.code == KeyCode::Enter
}

/// Returns normalized text for raw line-break character key events emitted by paste streams.
pub(super) fn line_break_char_key(key: &KeyEvent) -> Option<&'static str> {
    match key.code {
        KeyCode::Char('\n' | '\r') => Some("\n"),
        _ => None,
    }
}

/// Returns true when a key event is a Shift+arrow selection chord.
fn is_shift_arrow(key: &KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::SHIFT)
        && matches!(
            key.code,
            KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down
        )
}

/// Returns true when a key event is a specific Ctrl+character chord.
pub(super) fn is_ctrl_char(key: &KeyEvent, expected: char) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char(character) if character.eq_ignore_ascii_case(&expected))
}
