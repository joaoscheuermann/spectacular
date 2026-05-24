mod command;
mod key;
mod mouse;
mod paste;
mod selection;

use crate::action::ChatTuiAction;
use crate::runtime::{system_clipboard, ClipboardService, PasteBurst};
use crate::session::PromptState;
use crate::state::State;
use crate::view::{transcript_layout_snapshot, ViewAction, ViewState};
use iocraft::prelude::{KeyEventKind, TerminalEvent};
use std::time::{Duration, Instant};

pub const SPINNER_TICK_INTERVAL: Duration = Duration::from_millis(90);
pub const MAX_PASTE_BYTES: usize = 1_000_000;

/// Effect requested by local TUI event handling without performing side effects directly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventEffect {
    Action(Box<ChatTuiAction>),
    ViewAction(Box<ViewAction>),
    RequestExit,
}

/// Converts one IOCraft terminal event into reducer actions or outer-shell requests.
pub fn effects(state: &State, event: TerminalEvent) -> Vec<EventEffect> {
    let mut paste_burst = PasteBurst::default();
    match event {
        TerminalEvent::Key(key) => {
            let mut clipboard = if key::needs_system_clipboard(state, &key) {
                system_clipboard()
            } else {
                None
            };
            let clipboard = clipboard
                .as_mut()
                .map(|value| value.as_mut() as &mut dyn ClipboardService);
            key::effects(state, key, clipboard, &mut paste_burst, Instant::now())
        }
        TerminalEvent::Paste(value) => paste::terminal_effects(state, &value, &mut paste_burst),
        TerminalEvent::FullscreenMouse(mouse) => mouse::effects(state, mouse),
        TerminalEvent::Resize(width, height) => resize_effects(width, height),
        _ => Vec::new(),
    }
}

/// Converts one terminal event using an injected clipboard service for testable copy/paste behavior.
pub fn effects_with_clipboard(
    state: &State,
    event: TerminalEvent,
    clipboard: Option<&mut dyn ClipboardService>,
) -> Vec<EventEffect> {
    let mut paste_burst = PasteBurst::default();
    effects_with_clipboard_and_paste(state, event, clipboard, &mut paste_burst)
}

/// Converts one terminal event using caller-owned clipboard and paste-burst state.
pub(crate) fn effects_with_clipboard_and_paste(
    state: &State,
    event: TerminalEvent,
    clipboard: Option<&mut dyn ClipboardService>,
    paste_burst: &mut PasteBurst,
) -> Vec<EventEffect> {
    match event {
        TerminalEvent::Key(key) => key::effects(state, key, clipboard, paste_burst, Instant::now()),
        TerminalEvent::Paste(value) => paste::terminal_effects(state, &value, paste_burst),
        TerminalEvent::FullscreenMouse(mouse) => mouse::effects(state, mouse),
        TerminalEvent::Resize(width, height) => resize_effects(width, height),
        _ => Vec::new(),
    }
}

/// Converts one terminal event using cached view-local transcript layout where available.
pub(crate) fn effects_with_clipboard_paste_and_view(
    state: &State,
    view: &mut ViewState,
    event: TerminalEvent,
    clipboard: Option<&mut dyn ClipboardService>,
    paste_burst: &mut PasteBurst,
) -> Vec<EventEffect> {
    match event {
        TerminalEvent::Key(key_event) => {
            if !is_key_release(key_event.kind)
                && key::is_ctrl_char(&key_event, 'c')
                && state.app_selection.has_selection()
            {
                paste_burst.clear();
                let layout = transcript_layout_snapshot(state, view);
                return paste::copy_rendered_selection_effects_with_layout(
                    state,
                    &layout.layout,
                    clipboard,
                );
            }

            key::effects(state, key_event, clipboard, paste_burst, Instant::now())
        }
        TerminalEvent::Paste(value) => paste::terminal_effects(state, &value, paste_burst),
        TerminalEvent::FullscreenMouse(mouse) => {
            let layout = transcript_layout_snapshot(state, view);
            mouse::effects_with_layout(state, &layout.layout, mouse)
        }
        TerminalEvent::Resize(width, height) => resize_effects(width, height),
        _ => Vec::new(),
    }
}

/// Returns the effects emitted by the fixed-cadence spinner timer source.
pub fn timer_tick_effects() -> Vec<EventEffect> {
    action_effects(ChatTuiAction::SpinnerTick)
}

/// Wraps a reducer action into an event effect without inflating enum size.
pub(super) fn action_effects(action: ChatTuiAction) -> Vec<EventEffect> {
    vec![EventEffect::Action(Box::new(action))]
}

/// Wraps a view-local action into an event effect.
pub(super) fn view_action_effects(action: ViewAction) -> Vec<EventEffect> {
    vec![EventEffect::ViewAction(Box::new(action))]
}

fn resize_effects(width: u16, height: u16) -> Vec<EventEffect> {
    vec![
        EventEffect::Action(Box::new(ChatTuiAction::Resize { width, height })),
        EventEffect::ViewAction(Box::new(ViewAction::Resize { width, height })),
    ]
}

/// Builds a reducer-owned, prompt-local input notice effect.
pub(super) fn input_notice_effect(message: impl Into<String>) -> Vec<EventEffect> {
    action_effects(ChatTuiAction::InputNoticeReported {
        message: message.into(),
    })
}

/// Applies a local prompt edit and returns a PromptChanged action when state changed.
pub(super) fn prompt_change_effect<F>(state: &State, edit: F) -> Vec<EventEffect>
where
    F: FnOnce(&mut PromptState),
{
    let mut prompt = state.session.prompt.clone();
    edit(&mut prompt);
    prompt_changed_if_needed(state, prompt)
}

/// Returns a prompt changed action only when prompt state differs.
pub(super) fn prompt_changed_if_needed(state: &State, prompt: PromptState) -> Vec<EventEffect> {
    if prompt == state.session.prompt {
        return Vec::new();
    }

    action_effects(ChatTuiAction::PromptChanged(prompt))
}

/// Returns true when an event is a key release ignored by prompt routing.
pub(super) fn is_key_release(kind: KeyEventKind) -> bool {
    kind == KeyEventKind::Release
}
