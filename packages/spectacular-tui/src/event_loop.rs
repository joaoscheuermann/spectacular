use crate::action::ChatTuiAction;
use crate::ids::TranscriptItemId;
use crate::session::PromptState;
use crate::state::State;
use iocraft::prelude::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind, TerminalEvent,
};
use std::time::Duration;

pub const TUI_SPINNER_TICK_INTERVAL: Duration = Duration::from_millis(90);
const MOUSE_SCROLL_LINES: i32 = 3;

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
        TerminalEvent::Resize(_, _) => vec![EventEffect::Action(ChatTuiAction::Resize)],
        TerminalEvent::FullscreenMouse(mouse) => mouse_scroll_effect(mouse.kind)
            .into_iter()
            .map(ChatTuiAction::ScrollTranscript)
            .map(EventEffect::Action)
            .collect(),
        _ => Vec::new(),
    }
}

/// Returns the effects emitted by the fixed-cadence timer source.
pub fn tui_timer_tick_effects() -> Vec<EventEffect> {
    vec![EventEffect::Action(ChatTuiAction::SpinnerTick)]
}

/// Converts one key event into a reducer action or shell-level exit request.
fn key_effects(state: &State, key: KeyEvent) -> Vec<EventEffect> {
    if key.kind == KeyEventKind::Release {
        return Vec::new();
    }

    if is_ctrl_char(&key, 'c') {
        return ctrl_c_effects(state);
    }

    if is_newline_key(&key) {
        return prompt_change_effect(state, PromptState::insert_newline);
    }

    if key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    {
        return Vec::new();
    }

    let selecting = key.modifiers.contains(KeyModifiers::SHIFT);
    match key.code {
        KeyCode::Enter => submit_prompt_effects(state),
        KeyCode::Char(character) if !character.is_control() => {
            prompt_change_effect(state, |prompt| {
                prompt.insert_text(&character.to_string());
            })
        }
        KeyCode::Backspace => prompt_change_effect(state, PromptState::backspace),
        KeyCode::Delete => prompt_change_effect(state, PromptState::delete_forward),
        KeyCode::Left => prompt_change_effect(state, |prompt| prompt.move_left(selecting)),
        KeyCode::Right => prompt_change_effect(state, |prompt| prompt.move_right(selecting)),
        KeyCode::Up => prompt_change_effect(state, |prompt| prompt.move_up(selecting)),
        KeyCode::Down => prompt_change_effect(state, |prompt| prompt.move_down(selecting)),
        KeyCode::Home => prompt_change_effect(state, |prompt| prompt.move_to_start(selecting)),
        KeyCode::End => prompt_change_effect(state, |prompt| prompt.move_to_end(selecting)),
        _ => Vec::new(),
    }
}

/// Builds Ctrl+C behavior from current status without mutating state directly.
fn ctrl_c_effects(state: &State) -> Vec<EventEffect> {
    if state.status.is_cancellable() {
        return vec![EventEffect::Action(ChatTuiAction::CancelRun)];
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

/// Maps mouse wheel events to transcript scroll deltas.
fn mouse_scroll_effect(kind: MouseEventKind) -> Option<i32> {
    match kind {
        MouseEventKind::ScrollUp => Some(MOUSE_SCROLL_LINES),
        MouseEventKind::ScrollDown => Some(-MOUSE_SCROLL_LINES),
        _ => None,
    }
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
