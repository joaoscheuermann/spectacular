use crate::action::ChatTuiAction;
use crate::ids::TranscriptItemId;
use crate::session::{PromptState, SelectionPromptState};
use crate::state::State;
use arboard::Clipboard;
use iocraft::prelude::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, TerminalEvent};
use std::time::Duration;

pub const SPINNER_TICK_INTERVAL: Duration = Duration::from_millis(90);

/// Effect requested by local TUI event handling without performing side effects directly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventEffect {
    Action(Box<ChatTuiAction>),
    RequestExit,
}

/// Wraps a reducer action into an event effect without inflating enum size.
fn action_effects(action: ChatTuiAction) -> Vec<EventEffect> {
    vec![EventEffect::Action(Box::new(action))]
}

/// Converts one IOCraft terminal event into reducer actions or outer-shell requests.
pub fn effects(state: &State, event: TerminalEvent) -> Vec<EventEffect> {
    match event {
        TerminalEvent::Key(key) => key_effects(state, key),
        _ => Vec::new(),
    }
}

/// Returns the effects emitted by the fixed-cadence spinner timer source.
pub fn timer_tick_effects() -> Vec<EventEffect> {
    action_effects(ChatTuiAction::SpinnerTick)
}

/// Converts one key event into a reducer action or shell-level exit request.
fn key_effects(state: &State, key: KeyEvent) -> Vec<EventEffect> {
    if key.kind == KeyEventKind::Release {
        return Vec::new();
    }

    if state.selection.is_some() {
        return selection_key_effects(state, key);
    }

    if is_ctrl_char(&key, 'c') {
        return ctrl_c_effects(state);
    }
    if is_ctrl_char(&key, 'x') {
        return cut_effects(state);
    }
    if is_ctrl_char(&key, 'v') {
        return paste_effects(state);
    }
    if is_ctrl_char(&key, 'z') {
        return prompt_change_effect(state, PromptState::undo);
    }
    if is_ctrl_char(&key, 'r') {
        return prompt_change_effect(state, PromptState::redo);
    }
    if is_ctrl_char(&key, 'a') {
        return prompt_change_effect(state, PromptState::select_all);
    }
    if is_ctrl_char(&key, 'u') {
        return prompt_change_effect(state, PromptState::kill_to_line_start);
    }
    if is_ctrl_char(&key, 'k') {
        return prompt_change_effect(state, PromptState::kill_to_line_end);
    }
    if is_ctrl_char(&key, 'y') {
        return prompt_change_effect(state, PromptState::yank);
    }
    if is_submit_key(&key) {
        return submit_prompt_effects(state);
    }
    if is_line_break_key(&key) {
        return prompt_change_effect(state, PromptState::insert_newline);
    }

    let selecting = is_shift_arrow(&key) || key.modifiers.contains(KeyModifiers::SHIFT);
    let by_word = key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
    match key.code {
        KeyCode::Tab => accept_slash_completion_effects(state),
        KeyCode::Esc => prompt_change_effect(state, PromptState::escape),
        KeyCode::Char(' ') => space_effects(state),
        KeyCode::Char(character) if should_insert_char(&key, character) => {
            prompt_change_effect(state, |prompt| {
                prompt.insert_character(character);
            })
        }
        KeyCode::Backspace if by_word => {
            prompt_change_effect(state, PromptState::delete_previous_word)
        }
        KeyCode::Delete if by_word => prompt_change_effect(state, PromptState::delete_next_word),
        KeyCode::Backspace => prompt_change_effect(state, PromptState::backspace),
        KeyCode::Delete => prompt_change_effect(state, PromptState::delete_forward),
        KeyCode::Left if by_word => {
            prompt_change_effect(state, |prompt| prompt.move_word_left(selecting))
        }
        KeyCode::Right if by_word => {
            prompt_change_effect(state, |prompt| prompt.move_word_right(selecting))
        }
        KeyCode::Left => prompt_change_effect(state, |prompt| prompt.move_left(selecting)),
        KeyCode::Right => prompt_change_effect(state, |prompt| prompt.move_right(selecting)),
        KeyCode::Up => prompt_up_effects(state, selecting),
        KeyCode::Down => prompt_down_effects(state, selecting),
        KeyCode::Home if key.modifiers.contains(KeyModifiers::SHIFT) => {
            prompt_change_effect(state, |prompt| prompt.move_to_start(selecting))
        }
        KeyCode::End if key.modifiers.contains(KeyModifiers::SHIFT) => {
            prompt_change_effect(state, |prompt| prompt.move_to_end(selecting))
        }
        KeyCode::Home if key.modifiers.contains(KeyModifiers::CONTROL) => {
            prompt_change_effect(state, |prompt| prompt.move_to_start(selecting))
        }
        KeyCode::End if key.modifiers.contains(KeyModifiers::CONTROL) => {
            prompt_change_effect(state, |prompt| prompt.move_to_end(selecting))
        }
        KeyCode::Home => prompt_change_effect(state, |prompt| prompt.move_line_start(selecting)),
        KeyCode::End => prompt_change_effect(state, |prompt| prompt.move_line_end(selecting)),
        _ => Vec::new(),
    }
}

/// Builds Ctrl+C behavior from current status without mutating state directly.
fn ctrl_c_effects(state: &State) -> Vec<EventEffect> {
    if state.status.is_cancellable() {
        return action_effects(ChatTuiAction::CancelRun);
    }

    if let Some(mut prompt) = state
        .session
        .prompt
        .selection_range()
        .map(|_| state.session.prompt.clone())
    {
        if let Some(value) = prompt.copy_selection() {
            if !set_clipboard_text(value) {
                return Vec::new();
            }
            return prompt_changed_if_needed(state, prompt);
        }
    }

    if !state.session.prompt.is_empty() {
        return action_effects(ChatTuiAction::PromptChanged(PromptState::empty()));
    }

    vec![EventEffect::RequestExit]
}

/// Cuts selected prompt text to the native clipboard.
fn cut_effects(state: &State) -> Vec<EventEffect> {
    let mut prompt = state.session.prompt.clone();
    let Some(value) = prompt.cut_selection() else {
        return Vec::new();
    };

    if !set_clipboard_text(value) {
        return Vec::new();
    }

    prompt_changed_if_needed(state, prompt)
}

/// Pastes native clipboard text into the prompt when available.
fn paste_effects(state: &State) -> Vec<EventEffect> {
    let Some(value) = get_clipboard_text() else {
        return Vec::new();
    };

    prompt_change_effect(state, |prompt| prompt.insert_paste(&value))
}

/// Builds a prompt submission action for non-empty prompt text.
fn submit_prompt_effects(state: &State) -> Vec<EventEffect> {
    let text = state.session.prompt.text().trim().to_owned();
    if text.is_empty() {
        return Vec::new();
    }

    action_effects(ChatTuiAction::SubmitPrompt {
        id: next_local_prompt_id(state),
        text,
    })
}

/// Applies a local prompt edit and returns a PromptChanged action when state changed.
fn prompt_change_effect<F>(state: &State, edit: F) -> Vec<EventEffect>
where
    F: FnOnce(&mut PromptState),
{
    let mut prompt = state.session.prompt.clone();
    edit(&mut prompt);
    prompt_changed_if_needed(state, prompt)
}

/// Returns a prompt changed action only when prompt state differs.
fn prompt_changed_if_needed(state: &State, prompt: PromptState) -> Vec<EventEffect> {
    if prompt == state.session.prompt {
        return Vec::new();
    }

    action_effects(ChatTuiAction::PromptChanged(prompt))
}

/// Accepts the selected slash command completion into prompt text when available.
fn accept_slash_completion_effects(state: &State) -> Vec<EventEffect> {
    let suggestions = crate::prompt::slash_suggestions(&state.session.prompt, &state.commands);
    let Some(command) = suggestions
        .get(state.session.prompt.selected_completion)
        .or_else(|| suggestions.first())
    else {
        return Vec::new();
    };

    prompt_change_effect(state, |prompt| {
        prompt.accept_command_completion(command);
    })
}

/// Accepts a slash completion with Space, otherwise inserts a literal space.
fn space_effects(state: &State) -> Vec<EventEffect> {
    let accepted = accept_slash_completion_effects(state);
    if !accepted.is_empty() {
        return accepted;
    }

    prompt_change_effect(state, |prompt| prompt.insert_text(" "))
}

/// Moves through slash completions before falling back to prompt cursor up movement.
fn prompt_up_effects(state: &State, selecting: bool) -> Vec<EventEffect> {
    if !selecting
        && !crate::prompt::slash_suggestions(&state.session.prompt, &state.commands).is_empty()
    {
        return prompt_change_effect(state, PromptState::select_previous_completion);
    }

    prompt_change_effect(state, |prompt| {
        prompt.move_vertical_with_width(-1, selecting, state.prompt_layout.content_width);
    })
}

/// Moves through slash completions before falling back to prompt cursor down movement.
fn prompt_down_effects(state: &State, selecting: bool) -> Vec<EventEffect> {
    let count = crate::prompt::slash_suggestions(&state.session.prompt, &state.commands).len();
    if !selecting && count > 0 {
        return prompt_change_effect(state, |prompt| prompt.select_next_completion(count));
    }

    prompt_change_effect(state, |prompt| {
        prompt.move_vertical_with_width(1, selecting, state.prompt_layout.content_width);
    })
}

/// Handles one key event while a modal selection prompt is active.
fn selection_key_effects(state: &State, key: KeyEvent) -> Vec<EventEffect> {
    if is_ctrl_char(&key, 'c') {
        return vec![EventEffect::RequestExit];
    }

    let Some(selection) = state.selection.as_ref() else {
        return Vec::new();
    };
    match key.code {
        KeyCode::Enter | KeyCode::Char('\n' | '\r') if key.modifiers == KeyModifiers::NONE => {
            selection_submit_effect(selection)
        }
        KeyCode::Esc => selection_escape_effect(selection),
        KeyCode::Tab => selection_change_effect(selection, |selection| {
            selection.toggle_comment_mode();
        }),
        KeyCode::Up | KeyCode::Char('k') => {
            selection_change_effect(selection, SelectionPromptState::select_previous)
        }
        KeyCode::Down | KeyCode::Char('j') => {
            selection_change_effect(selection, SelectionPromptState::select_next)
        }
        KeyCode::Backspace => selection_change_effect(selection, SelectionPromptState::backspace),
        KeyCode::Delete => selection_change_effect(selection, SelectionPromptState::delete_forward),
        KeyCode::Char(character) if should_insert_char(&key, character) => {
            selection_change_effect(selection, |selection| {
                selection.insert_text(&character.to_string());
            })
        }
        _ => Vec::new(),
    }
}

/// Handles Escape by leaving comment mode or cancelling the modal selection prompt.
fn selection_escape_effect(selection: &SelectionPromptState) -> Vec<EventEffect> {
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
fn selection_submit_effect(selection: &SelectionPromptState) -> Vec<EventEffect> {
    let Some(answer) = selection.answer() else {
        return Vec::new();
    };

    action_effects(ChatTuiAction::SelectionPromptSubmitted(answer))
}

/// Applies a local selection prompt edit and returns an action when state changed.
fn selection_change_effect<F>(selection: &SelectionPromptState, edit: F) -> Vec<EventEffect>
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

/// Returns true for printable key events without control or alt chords.
fn should_insert_char(key: &KeyEvent, character: char) -> bool {
    !character.is_control()
        && !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
}

/// Allocates a deterministic local prompt ID from current semantic transcript length.
fn next_local_prompt_id(state: &State) -> TranscriptItemId {
    TranscriptItemId::new(format!(
        "local-prompt-{}",
        state.session.transcript.len().saturating_add(1)
    ))
}

/// Returns true when a key event should submit the active prompt.
fn is_submit_key(key: &KeyEvent) -> bool {
    key.code == KeyCode::Enter && key.modifiers.contains(KeyModifiers::CONTROL)
        || matches!(key.code, KeyCode::Char('\n' | '\r'))
            && key.modifiers.contains(KeyModifiers::CONTROL)
}

/// Returns true when a key event should insert a multiline prompt line break.
fn is_line_break_key(key: &KeyEvent) -> bool {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return false;
    }

    key.code == KeyCode::Enter || matches!(key.code, KeyCode::Char('\n' | '\r'))
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
fn is_ctrl_char(key: &KeyEvent, expected: char) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char(character) if character.eq_ignore_ascii_case(&expected))
}

/// Writes text to the platform clipboard and reports whether native synchronization succeeded.
fn set_clipboard_text(value: String) -> bool {
    Clipboard::new()
        .and_then(|mut clipboard| clipboard.set_text(value))
        .is_ok()
}

/// Best-effort read from the platform clipboard.
fn get_clipboard_text() -> Option<String> {
    Clipboard::new().ok()?.get_text().ok()
}
