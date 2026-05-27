use crate::action::{ChatTuiAction, SelectionPromptAnswer};
use crate::ids::TranscriptItemId;
use crate::runtime::{
    apply_controller_action, apply_event_effects, effects_with_clipboard_paste_and_view,
    system_clipboard, ClipboardService, PasteBurst,
};
use crate::state::State;
use crate::view::{materialize_state, ViewState};
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
        apply_controller_action(&mut self.state, &mut self.view, action);
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
            effects_with_clipboard_paste_and_view(
                &frame,
                &mut self.view,
                event,
                clipboard,
                &mut self.paste_burst,
            )
        };
        for intent in apply_event_effects(&mut self.state, &mut self.view, effects) {
            self.emit_intent(intent);
        }
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
