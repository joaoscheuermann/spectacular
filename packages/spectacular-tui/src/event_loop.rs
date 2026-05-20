use crate::action::ChatTuiAction;
use crate::ids::TranscriptItemId;
use crate::session::{PromptState, SelectionPromptState};
use crate::state::State;
use iocraft::prelude::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, TerminalEvent};
use std::time::Duration;

pub const TUI_SPINNER_TICK_INTERVAL: Duration = Duration::from_millis(90);

/// Effect requested by local TUI event handling without performing side effects directly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventEffect {
    Action(ChatTuiAction),
    RequestExit,
}

/// Converts one IOCraft terminal event into reducer actions or outer-shell requests.
pub fn tui_event_effects(state: &State, event: TerminalEvent) -> Vec<EventEffect> {
    match event {
        TerminalEvent::Key(key) => key_effects(state, key),
        _ => Vec::new(),
    }
}

/// Returns the effects emitted by the fixed-cadence spinner timer source.
pub fn tui_timer_tick_effects() -> Vec<EventEffect> {
    vec![EventEffect::Action(ChatTuiAction::SpinnerTick)]
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
    if is_newline_key(&key) {
        return prompt_change_effect(state, PromptState::insert_newline);
    }

    let selecting = key.modifiers.contains(KeyModifiers::SHIFT);
    let by_word = key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
    match key.code {
        KeyCode::Enter => prompt_enter_effects(state),
        KeyCode::Tab => accept_slash_completion_effects(state),
        KeyCode::Esc => prompt_change_effect(state, PromptState::escape),
        KeyCode::Char(' ') => space_effects(state),
        KeyCode::Char(character) if should_insert_char(&key, character) => {
            prompt_change_effect(state, |prompt| {
                prompt.insert_text(&character.to_string());
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
        return vec![EventEffect::Action(ChatTuiAction::CancelRun)];
    }
    if !state.session.prompt.text.is_empty() {
        return vec![EventEffect::Action(ChatTuiAction::PromptChanged(
            PromptState::empty(),
        ))];
    }

    vec![EventEffect::RequestExit]
}

/// Builds a prompt submission action for non-empty prompt text.
fn submit_prompt_effects(state: &State) -> Vec<EventEffect> {
    let text = state.session.prompt.text.trim().to_owned();
    if text.is_empty() {
        return Vec::new();
    }

    vec![EventEffect::Action(ChatTuiAction::SubmitPrompt {
        id: next_local_prompt_id(state),
        text,
    })]
}

/// Applies a local prompt edit and returns a PromptChanged action when state changed.
fn prompt_change_effect<F>(state: &State, edit: F) -> Vec<EventEffect>
where
    F: FnOnce(&mut PromptState),
{
    let mut prompt = state.session.prompt.clone();
    edit(&mut prompt);
    if prompt == state.session.prompt {
        return Vec::new();
    }

    vec![EventEffect::Action(ChatTuiAction::PromptChanged(prompt))]
}

/// Handles Enter after slash completion has a chance to accept a candidate.
fn prompt_enter_effects(state: &State) -> Vec<EventEffect> {
    let accepted = accept_slash_completion_effects(state);
    if !accepted.is_empty() {
        return accepted;
    }

    submit_prompt_effects(state)
}

/// Accepts the selected slash command completion into prompt text when available.
fn accept_slash_completion_effects(state: &State) -> Vec<EventEffect> {
    let suggestions =
        crate::prompt_state::slash_suggestions(&state.session.prompt, &state.commands);
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
    if !crate::prompt_state::slash_suggestions(&state.session.prompt, &state.commands).is_empty() {
        return prompt_change_effect(state, PromptState::select_previous_completion);
    }

    prompt_change_effect(state, |prompt| prompt.move_up(selecting))
}

/// Moves through slash completions before falling back to prompt cursor down movement.
fn prompt_down_effects(state: &State, selecting: bool) -> Vec<EventEffect> {
    let count =
        crate::prompt_state::slash_suggestions(&state.session.prompt, &state.commands).len();
    if count > 0 {
        return prompt_change_effect(state, |prompt| prompt.select_next_completion(count));
    }

    prompt_change_effect(state, |prompt| prompt.move_down(selecting))
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
        return vec![EventEffect::Action(ChatTuiAction::SelectionPromptCancelled)];
    }

    if &next == selection {
        return Vec::new();
    }

    vec![EventEffect::Action(ChatTuiAction::SelectionPromptChanged(
        Some(next),
    ))]
}

/// Builds a selection prompt submit action when the selected answer is valid.
fn selection_submit_effect(selection: &SelectionPromptState) -> Vec<EventEffect> {
    let Some(answer) = selection.answer() else {
        return Vec::new();
    };

    vec![EventEffect::Action(
        ChatTuiAction::SelectionPromptSubmitted(answer),
    )]
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

    vec![EventEffect::Action(ChatTuiAction::SelectionPromptChanged(
        Some(next),
    ))]
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

/// Returns true when a key event should insert a multiline prompt line break.
fn is_newline_key(key: &KeyEvent) -> bool {
    key.code == KeyCode::Enter
        && (key.modifiers.contains(KeyModifiers::ALT)
            || key.modifiers.contains(KeyModifiers::CONTROL)
            || key.modifiers.contains(KeyModifiers::SHIFT))
        || is_ctrl_char(key, 'j')
}

/// Returns true when a key event is a specific Ctrl+character chord.
fn is_ctrl_char(key: &KeyEvent, expected: char) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char(character) if character.eq_ignore_ascii_case(&expected))
}
