use crate::action::{ChatTuiAction, SelectionPromptAnswer};
use crate::ids::TranscriptItemId;
use crate::reducer::reduce;
use crate::runtime::{
    effects_with_clipboard_and_paste, system_clipboard, ClipboardService, EventEffect, PasteBurst,
};
use crate::state::State;
use crate::view::{
    apply_view_action, clear_selection, materialize_state, preserve_review_position_for_growth,
    total_transcript_rows, ViewState,
};
use iocraft::prelude::TerminalEvent;
use tokio::sync::mpsc;

const RUNTIME_INTENT_BUFFER: usize = 16;

/// User intent emitted by the TUI shell for controller-owned runtime side effects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Intent {
    SubmitPrompt { id: TranscriptItemId, text: String },
    SelectionPromptSubmitted(SelectionPromptAnswer),
    SelectionPromptCancelled,
    CancelRun,
    RequestExit,
}

/// Framework-independent controller for TUI state and runtime intents.
pub struct Shell {
    state: State,
    view: ViewState,
    intent_sender: mpsc::Sender<Intent>,
    clipboard: Option<Box<dyn ClipboardService>>,
    paste_burst: PasteBurst,
}

impl Shell {
    /// Creates a runtime shell and the receiver for emitted user intents.
    pub fn new(state: State) -> (Self, mpsc::Receiver<Intent>) {
        Self::with_clipboard(state, system_clipboard())
    }

    /// Creates a runtime shell without opening the platform clipboard.
    pub(crate) fn new_without_clipboard(state: State) -> (Self, mpsc::Receiver<Intent>) {
        Self::with_clipboard(state, None)
    }

    /// Creates a runtime shell with caller-owned clipboard access.
    fn with_clipboard(
        state: State,
        clipboard: Option<Box<dyn ClipboardService>>,
    ) -> (Self, mpsc::Receiver<Intent>) {
        let (intent_sender, intent_receiver) = mpsc::channel(RUNTIME_INTENT_BUFFER);
        (
            Self {
                view: ViewState::from_state(&state),
                state,
                intent_sender,
                clipboard,
                paste_burst: PasteBurst::default(),
            },
            intent_receiver,
        )
    }

    /// Returns the current reducer-owned TUI state.
    pub fn state(&self) -> &State {
        &self.state
    }

    /// Returns the current local view state.
    pub fn view(&self) -> &ViewState {
        &self.view
    }

    /// Applies a controller-originated action to the TUI reducer.
    pub fn apply_action(&mut self, action: ChatTuiAction) {
        let old_rows = total_transcript_rows(&materialize_state(&self.state, &self.view));
        reduce(&mut self.state, action);
        let new_rows = total_transcript_rows(&materialize_state(&self.state, &self.view));
        preserve_review_position_for_growth(&mut self.view, old_rows, new_rows);
        self.sync_view_mirror();
    }

    /// Replaces the clipboard service used by local copy/paste handling.
    pub fn set_clipboard(&mut self, clipboard: Option<Box<dyn ClipboardService>>) {
        self.clipboard = clipboard;
    }

    /// Converts one terminal event into reducer state and runtime intents.
    pub fn apply_terminal_event(&mut self, event: TerminalEvent) {
        let effects = {
            let clipboard = self
                .clipboard
                .as_mut()
                .map(|value| value.as_mut() as &mut dyn ClipboardService);
            let frame = materialize_state(&self.state, &self.view);
            effects_with_clipboard_and_paste(&frame, event, clipboard, &mut self.paste_burst)
        };
        for effect in effects {
            self.apply_event_effect(effect);
        }
    }

    /// Converts one terminal event using caller-owned clipboard and paste-burst state.
    pub(crate) fn apply_terminal_event_with_clipboard_and_paste(
        &mut self,
        event: TerminalEvent,
        clipboard: Option<&mut dyn ClipboardService>,
        paste_burst: &mut PasteBurst,
    ) {
        let frame = materialize_state(&self.state, &self.view);
        let effects = effects_with_clipboard_and_paste(&frame, event, clipboard, paste_burst);
        for effect in effects {
            self.apply_event_effect(effect);
        }
    }

    /// Applies one local event effect without performing runtime work directly.
    fn apply_event_effect(&mut self, effect: EventEffect) {
        match effect {
            EventEffect::Action(action) => self.apply_user_action(*action),
            EventEffect::ViewAction(action) => self.apply_view_action(*action),
            EventEffect::RequestExit => self.emit_intent(Intent::RequestExit),
        }
    }

    /// Applies a user action and emits the matching runtime intent when needed.
    fn apply_user_action(&mut self, action: ChatTuiAction) {
        let intent = intent_for_action(&action);
        let should_clear_selection = clears_rendered_selection(&action);
        let should_reset_view = resets_view(&action);
        let old_rows = total_transcript_rows(&materialize_state(&self.state, &self.view));
        reduce(&mut self.state, action);
        let new_rows = total_transcript_rows(&materialize_state(&self.state, &self.view));
        preserve_review_position_for_growth(&mut self.view, old_rows, new_rows);
        if should_reset_view {
            self.view.reset_for_session();
        } else if should_clear_selection {
            clear_selection(&mut self.view);
        }
        self.sync_view_mirror();
        if let Some(intent) = intent {
            self.emit_intent(intent);
        }
    }

    fn apply_view_action(&mut self, action: crate::view::ViewAction) {
        apply_view_action(&mut self.view, &self.state, action);
        self.sync_view_mirror();
    }

    fn sync_view_mirror(&mut self) {
        self.state = materialize_state(&self.state, &self.view);
    }

    /// Emits an intent without blocking render/event handling.
    fn emit_intent(&self, intent: Intent) {
        let _ = self.intent_sender.try_send(intent);
    }
}

fn clears_rendered_selection(action: &ChatTuiAction) -> bool {
    matches!(
        action,
        ChatTuiAction::PromptChanged(_)
            | ChatTuiAction::SubmitPrompt { .. }
            | ChatTuiAction::SelectionPromptChanged(_)
            | ChatTuiAction::SelectionPromptSubmitted(_)
            | ChatTuiAction::SelectionPromptCancelled
    )
}

fn resets_view(action: &ChatTuiAction) -> bool {
    matches!(
        action,
        ChatTuiAction::SessionChanged { .. } | ChatTuiAction::SessionCreated { .. }
    )
}

/// Converts reducer-visible user actions into controller runtime intents.
fn intent_for_action(action: &ChatTuiAction) -> Option<Intent> {
    match action {
        ChatTuiAction::SubmitPrompt { id, text } => Some(Intent::SubmitPrompt {
            id: id.clone(),
            text: text.clone(),
        }),
        ChatTuiAction::SelectionPromptSubmitted(answer) => {
            Some(Intent::SelectionPromptSubmitted(answer.clone()))
        }
        ChatTuiAction::SelectionPromptCancelled => Some(Intent::SelectionPromptCancelled),
        ChatTuiAction::CancelRun => Some(Intent::CancelRun),
        _ => None,
    }
}
