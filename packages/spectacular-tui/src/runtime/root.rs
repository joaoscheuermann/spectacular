use crate::components::App;
use crate::reducer::reduce;
use crate::runtime::{system_clipboard, ClipboardService, Intent, PasteBurst, Shell};
use crate::session::PromptState;
use crate::state::State as TuiState;
use crate::view::{
    apply_view_action, materialize_state, preserve_review_position_for_growth,
    total_transcript_rows, ViewAction, ViewState,
};
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
    let local_view = hooks.use_state(|| ViewState::from_state(&initial_state));
    let local_state = hooks.use_state(|| initial_state);
    let exit_requested = hooks.use_state(|| false);
    let clipboard = hooks.use_ref(|| Mutex::new(system_clipboard()));
    let paste_burst = hooks.use_ref(|| Mutex::new(PasteBurst::default()));
    let (terminal_width, terminal_height) = hooks.use_terminal_size();

    synchronize_runtime_state(
        &mut hooks,
        local_state,
        local_view,
        current_prompt,
        state_receiver,
    );
    drive_selection_auto_scroll(&mut hooks, local_state, local_view, current_prompt);
    emit_terminal_intents(
        &mut hooks,
        local_state,
        local_view,
        current_prompt,
        exit_requested,
        clipboard,
        paste_burst,
        terminal_width,
        terminal_height,
        intent_sender,
        cancellation_sender,
        selection_sender,
    );

    let state = state_with_current_prompt_and_view(
        &local_state.read(),
        &current_prompt.read(),
        &local_view.read(),
    );
    if exit_requested.get() || state.exit_requested {
        system.exit();
    }

    element!(App(state))
}

/// Continues transcript selection autoscroll while the pointer is held past a viewport edge.
fn drive_selection_auto_scroll(
    hooks: &mut Hooks,
    local_state: iocraft::prelude::State<TuiState>,
    mut local_view: iocraft::prelude::State<ViewState>,
    current_prompt: iocraft::prelude::State<PromptState>,
) {
    hooks.use_future(async move {
        loop {
            tokio::time::sleep(crate::runtime::SPINNER_TICK_INTERVAL).await;
            let mut view = local_view.read().clone();
            let state = state_with_current_prompt_and_view(
                &local_state.read(),
                &current_prompt.read(),
                &view,
            );
            let Some(edge) = view.app_selection.viewport_edge else {
                continue;
            };
            if !view.app_selection.dragging {
                continue;
            }

            apply_view_action(
                &mut view,
                &state,
                ViewAction::RenderedSelectionAutoScroll(edge),
            );
            local_view.set(view);
        }
    });
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
    mut local_view: iocraft::prelude::State<ViewState>,
    mut current_prompt: iocraft::prelude::State<PromptState>,
    state_receiver: Arc<Mutex<mpsc::UnboundedReceiver<TuiState>>>,
) {
    hooks.use_future(async move {
        loop {
            apply_state_updates(
                &mut local_state,
                &mut local_view,
                &mut current_prompt,
                &state_receiver,
            );
            tokio::time::sleep(std::time::Duration::from_millis(16)).await;
        }
    });
}

/// Applies all pending state snapshots from the runtime controller without clobbering local prompt edits.
fn apply_state_updates(
    local_state: &mut iocraft::prelude::State<TuiState>,
    local_view: &mut iocraft::prelude::State<ViewState>,
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
        let view_snapshot = local_view.read().clone();
        let prompt_snapshot = current_prompt.read().clone();
        let (state, view, prompt_reset) = merge_controller_state_and_view_update(
            &local_snapshot,
            &view_snapshot,
            state,
            &prompt_snapshot,
        );
        if let Some(prompt) = prompt_reset {
            current_prompt.set(prompt);
        }
        local_view.set(view);
        local_state.set(state);
    }
}

/// Returns render state with the prompt value owned by the interactive prompt component.
fn state_with_current_prompt(state: &TuiState, prompt: &PromptState) -> TuiState {
    let mut state = state.clone();
    state.session.prompt = prompt.clone();
    state
}

fn state_with_current_prompt_and_view(
    state: &TuiState,
    prompt: &PromptState,
    view: &ViewState,
) -> TuiState {
    materialize_state(&state_with_current_prompt(state, prompt), view)
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
    controller_state.app_selection = local_state.app_selection.clone();
    controller_state.scroll = local_state.scroll.clone();
    controller_state.exit_requested |= local_state.exit_requested;
    (controller_state, None)
}

/// Merges controller semantic state while preserving local same-session view state.
pub fn merge_controller_state_and_view_update(
    local_state: &TuiState,
    local_view: &ViewState,
    mut controller_state: TuiState,
    current_prompt: &PromptState,
) -> (TuiState, ViewState, Option<PromptState>) {
    if controller_state.session.id != local_state.session.id {
        let reset_prompt = controller_state.session.prompt.clone();
        let mut view = ViewState::from_state(&controller_state);
        view.reset_for_session();
        return (controller_state, view, Some(reset_prompt));
    }

    let old_rows = total_transcript_rows(&materialize_state(local_state, local_view));
    controller_state.session.prompt = current_prompt.clone();
    controller_state.input_notice = local_state.input_notice.clone();
    controller_state.exit_requested |= local_state.exit_requested;
    let mut view = local_view.clone();
    let new_rows = total_transcript_rows(&materialize_state(&controller_state, &view));
    preserve_review_position_for_growth(&mut view, old_rows, new_rows);
    (controller_state, view, None)
}

/// Registers terminal input handling that emits runtime intents without performing side effects.
fn emit_terminal_intents(
    hooks: &mut Hooks,
    local_state: iocraft::prelude::State<TuiState>,
    local_view: iocraft::prelude::State<ViewState>,
    current_prompt: iocraft::prelude::State<PromptState>,
    exit_requested: iocraft::prelude::State<bool>,
    clipboard: Ref<Mutex<Option<Box<dyn ClipboardService>>>>,
    paste_burst: Ref<Mutex<PasteBurst>>,
    terminal_width: u16,
    terminal_height: u16,
    intent_sender: mpsc::UnboundedSender<Intent>,
    cancellation_sender: mpsc::UnboundedSender<()>,
    selection_sender: mpsc::UnboundedSender<Intent>,
) {
    hooks.use_terminal_events({
        let mut local_state = local_state;
        let mut local_view = local_view;
        let mut current_prompt = current_prompt;
        let mut exit_requested = exit_requested;
        let clipboard = clipboard;
        let paste_burst = paste_burst;
        move |event| {
            let mut view = local_view.read().clone();
            let mut state = state_with_current_prompt_and_view(
                &local_state.read(),
                &current_prompt.read(),
                &view,
            );
            if terminal_width > 0 && terminal_height > 0 {
                reduce(
                    &mut state,
                    crate::action::ChatTuiAction::Resize {
                        width: terminal_width,
                        height: terminal_height,
                    },
                );
                apply_view_action(
                    &mut view,
                    &state,
                    ViewAction::Resize {
                        width: terminal_width,
                        height: terminal_height,
                    },
                );
                state = materialize_state(&state, &view);
            }
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
            let view = shell.view().clone();
            current_prompt.set(state.session.prompt.clone());
            local_state.set(state);
            local_view.set(view);
        }
    });
}
