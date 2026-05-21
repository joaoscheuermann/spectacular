use crate::components::App;
use crate::runtime::{system_clipboard, ClipboardService, Intent, PasteBurst, Shell};
use crate::session::PromptState;
use crate::state::State as TuiState;
use iocraft::prelude::*;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// Interactive IOCraft root that owns terminal hooks and renders the visual app.
#[component]
pub fn Root(mut hooks: Hooks, props: &RootProps) -> impl Into<AnyElement<'static>> {
    let initial_state = props
        .initial_state
        .as_ref()
        .expect("Root requires initial state")
        .clone();
    let intent_sender = props
        .intent_sender
        .as_ref()
        .expect("Root requires intent sender")
        .clone();
    let cancellation_sender = props
        .cancellation_sender
        .as_ref()
        .expect("Root requires cancellation sender")
        .clone();
    let selection_sender = props
        .selection_sender
        .as_ref()
        .expect("Root requires selection sender")
        .clone();
    let state_receiver = props
        .state_receiver
        .as_ref()
        .expect("Root requires state receiver")
        .clone();
    let mut system = hooks.use_context_mut::<SystemContext>();
    let current_prompt = hooks.use_state(|| initial_state.session.prompt.clone());
    let local_state = hooks.use_state(|| initial_state);
    let exit_requested = hooks.use_state(|| false);
    let clipboard = hooks.use_ref(|| Mutex::new(system_clipboard()));
    let paste_burst = hooks.use_ref(|| Mutex::new(PasteBurst::default()));

    synchronize_runtime_state(&mut hooks, local_state, current_prompt, state_receiver);
    emit_terminal_intents(
        &mut hooks,
        local_state,
        current_prompt,
        exit_requested,
        clipboard,
        paste_burst,
        intent_sender,
        cancellation_sender,
        selection_sender,
    );

    let state = state_with_current_prompt(&local_state.read(), &current_prompt.read());
    if exit_requested.get() || state.exit_requested {
        system.exit();
    }

    element!(App(state))
}

/// Props for the interactive IOCraft root component.
#[derive(Default, Props)]
pub struct RootProps {
    pub initial_state: Option<TuiState>,
    pub intent_sender: Option<mpsc::UnboundedSender<Intent>>,
    pub cancellation_sender: Option<mpsc::UnboundedSender<()>>,
    pub selection_sender: Option<mpsc::UnboundedSender<Intent>>,
    pub state_receiver: Option<Arc<Mutex<mpsc::UnboundedReceiver<TuiState>>>>,
}

/// Polls controller-published state snapshots and refreshes local IOCraft state.
fn synchronize_runtime_state(
    hooks: &mut Hooks,
    mut local_state: iocraft::prelude::State<TuiState>,
    mut current_prompt: iocraft::prelude::State<PromptState>,
    state_receiver: Arc<Mutex<mpsc::UnboundedReceiver<TuiState>>>,
) {
    hooks.use_future(async move {
        loop {
            apply_state_updates(&mut local_state, &mut current_prompt, &state_receiver);
            tokio::time::sleep(std::time::Duration::from_millis(16)).await;
        }
    });
}

/// Applies all pending state snapshots from the runtime controller without clobbering local prompt edits.
fn apply_state_updates(
    local_state: &mut iocraft::prelude::State<TuiState>,
    current_prompt: &mut iocraft::prelude::State<PromptState>,
    state_receiver: &Arc<Mutex<mpsc::UnboundedReceiver<TuiState>>>,
) {
    loop {
        let Ok(state) = state_receiver
            .lock()
            .expect("TUI state receiver lock poisoned")
            .try_recv()
        else {
            return;
        };
        let local_snapshot = local_state.read().clone();
        let prompt_snapshot = current_prompt.read().clone();
        let (state, prompt_reset) =
            merge_controller_state_update(&local_snapshot, state, &prompt_snapshot);
        if let Some(prompt) = prompt_reset {
            current_prompt.set(prompt);
        }
        local_state.set(state);
    }
}

/// Returns render state with the prompt value owned by the interactive prompt component.
fn state_with_current_prompt(state: &TuiState, prompt: &PromptState) -> TuiState {
    let mut state = state.clone();
    state.session.prompt = prompt.clone();
    state
}

/// Merges a controller snapshot while keeping same-session prompt input local to the TUI.
pub fn merge_controller_state_update(
    local_state: &TuiState,
    mut controller_state: TuiState,
    current_prompt: &PromptState,
) -> (TuiState, Option<PromptState>) {
    if controller_state.session.id != local_state.session.id {
        let reset_prompt = controller_state.session.prompt.clone();
        return (controller_state, Some(reset_prompt));
    }

    controller_state.session.prompt = current_prompt.clone();
    controller_state.input_notice = local_state.input_notice.clone();
    controller_state.exit_requested |= local_state.exit_requested;
    (controller_state, None)
}

/// Registers terminal input handling that emits runtime intents without performing side effects.
fn emit_terminal_intents(
    hooks: &mut Hooks,
    local_state: iocraft::prelude::State<TuiState>,
    current_prompt: iocraft::prelude::State<PromptState>,
    exit_requested: iocraft::prelude::State<bool>,
    clipboard: Ref<Mutex<Option<Box<dyn ClipboardService>>>>,
    paste_burst: Ref<Mutex<PasteBurst>>,
    intent_sender: mpsc::UnboundedSender<Intent>,
    cancellation_sender: mpsc::UnboundedSender<()>,
    selection_sender: mpsc::UnboundedSender<Intent>,
) {
    hooks.use_terminal_events({
        let mut local_state = local_state;
        let mut current_prompt = current_prompt;
        let mut exit_requested = exit_requested;
        let clipboard = clipboard;
        let paste_burst = paste_burst;
        move |event| {
            let state = state_with_current_prompt(&local_state.read(), &current_prompt.read());
            let (mut shell, mut intents) = Shell::new_without_clipboard(state);
            {
                let clipboard_ref = clipboard.read();
                let mut clipboard = clipboard_ref
                    .lock()
                    .expect("TUI clipboard service lock poisoned");
                let clipboard = clipboard
                    .as_mut()
                    .map(|value| value.as_mut() as &mut dyn ClipboardService);
                let paste_burst_ref = paste_burst.read();
                let mut paste_burst = paste_burst_ref
                    .lock()
                    .expect("TUI paste-burst lock poisoned");
                shell.apply_terminal_event_with_clipboard_and_paste(
                    event,
                    clipboard,
                    &mut paste_burst,
                );
            }
            while let Ok(intent) = intents.try_recv() {
                match intent {
                    Intent::RequestExit => {
                        exit_requested.set(true);
                        let _ = intent_sender.send(Intent::RequestExit);
                    }
                    Intent::CancelRun => {
                        let _ = cancellation_sender.send(());
                    }
                    Intent::SelectionPromptSubmitted(_) | Intent::SelectionPromptCancelled => {
                        let _ = selection_sender.send(intent);
                    }
                    intent => {
                        let _ = intent_sender.send(intent);
                    }
                }
            }
            let state = shell.state().clone();
            current_prompt.set(state.session.prompt.clone());
            local_state.set(state);
        }
    });
}
