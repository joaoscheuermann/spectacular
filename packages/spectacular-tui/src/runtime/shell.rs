use crate::action::{ChatTuiAction, SelectionPromptAnswer};
use crate::ids::TranscriptItemId;
use crate::reducer::reduce;
use crate::runtime::{effects, EventEffect};
use crate::state::State;
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
    intent_sender: mpsc::Sender<Intent>,
}

impl Shell {
    /// Creates a runtime shell and the receiver for emitted user intents.
    pub fn new(state: State) -> (Self, mpsc::Receiver<Intent>) {
        let (intent_sender, intent_receiver) = mpsc::channel(RUNTIME_INTENT_BUFFER);
        (
            Self {
                state,
                intent_sender,
            },
            intent_receiver,
        )
    }

    /// Returns the current reducer-owned TUI state.
    pub fn state(&self) -> &State {
        &self.state
    }

    /// Applies a controller-originated action to the TUI reducer.
    pub fn apply_action(&mut self, action: ChatTuiAction) {
        reduce(&mut self.state, action);
    }

    /// Converts one terminal event into reducer state and runtime intents.
    pub fn apply_terminal_event(&mut self, event: TerminalEvent) {
        for effect in effects(&self.state, event) {
            self.apply_event_effect(effect);
        }
    }

    /// Applies one local event effect without performing runtime work directly.
    fn apply_event_effect(&mut self, effect: EventEffect) {
        match effect {
            EventEffect::Action(action) => self.apply_user_action(*action),
            EventEffect::RequestExit => self.emit_intent(Intent::RequestExit),
        }
    }

    /// Applies a user action and emits the matching runtime intent when needed.
    fn apply_user_action(&mut self, action: ChatTuiAction) {
        let intent = intent_for_action(&action);
        reduce(&mut self.state, action);
        if let Some(intent) = intent {
            self.emit_intent(intent);
        }
    }

    /// Emits an intent without blocking render/event handling.
    fn emit_intent(&self, intent: Intent) {
        let _ = self.intent_sender.try_send(intent);
    }
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
