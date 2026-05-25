use super::controller::{Bootstrap, Controller};
use super::runner::TurnRunner;
use crate::chat::{ChatBootstrap, ChatError};
use iocraft::prelude::*;
use spectacular_llms::LlmDebugLogger;
use spectacular_tui::{root_element, Intent, State};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

/// Runs the production IOCraft render loop with the real Spectacular runtime controller.
pub(crate) async fn run(debug_logger: LlmDebugLogger) -> Result<(), ChatError> {
    let bootstrap = Bootstrap::from_chat_bootstrap(ChatBootstrap::new(debug_logger)?)?;
    run_with_controller(Controller::new(bootstrap)?).await
}

/// Coordinates IOCraft-rendered terminal UI with controller-owned async runtime work.
async fn run_with_controller<R>(controller: Controller<R>) -> Result<(), ChatError>
where
    R: TurnRunner + 'static,
{
    let (intent_sender, intent_receiver) = mpsc::unbounded_channel();
    let (cancellation_sender, cancellation_receiver) = mpsc::unbounded_channel();
    let (selection_sender, selection_receiver) = mpsc::unbounded_channel();
    let (state_sender, state_receiver) = mpsc::unbounded_channel();
    let initial_state = controller.state_snapshot();
    let state_receiver = Arc::new(Mutex::new(state_receiver));
    let runtime = tokio::runtime::Handle::current();
    let controller_loop = run_controller_loop(
        controller,
        intent_receiver,
        cancellation_receiver,
        selection_receiver,
        state_sender,
    );
    let render_task = tokio::task::spawn_blocking(move || {
        runtime.block_on(async move {
            root_element(
                initial_state,
                intent_sender,
                cancellation_sender,
                selection_sender,
                state_receiver,
            )
            .fullscreen()
            .ignore_ctrl_c()
            .await
        })
    });
    let (render_result, controller_result) = tokio::join!(render_task, controller_loop);
    let render_result = render_result.map_err(|error| ChatError::Session(error.to_string()))?;
    render_result.map_err(ChatError::Io)?;
    controller_result
}

/// Processes intents emitted by the IOCraft shell and publishes reducer snapshots for rendering.
pub(crate) async fn run_controller_loop<R>(
    mut controller: Controller<R>,
    mut intent_receiver: mpsc::UnboundedReceiver<Intent>,
    mut cancellation_receiver: mpsc::UnboundedReceiver<()>,
    mut selection_receiver: mpsc::UnboundedReceiver<Intent>,
    state_sender: mpsc::UnboundedSender<State>,
) -> Result<(), ChatError>
where
    R: TurnRunner,
{
    let mut worktree_refresh = tokio::time::interval(Duration::from_secs(2));

    loop {
        tokio::select! {
            intent = intent_receiver.recv() => {
                let Some(intent) = intent else {
                    return Ok(());
                };
                let should_exit = controller
                    .handle_intent_with_state_sender(
                        intent,
                        &state_sender,
                        &mut cancellation_receiver,
                        &mut selection_receiver,
                    )
                    .await?;
                let _ = state_sender.send(controller.state_snapshot());
                if should_exit {
                    return Ok(());
                }
            }
            _ = worktree_refresh.tick() => {
                controller.refresh_worktree_metadata(Some(&state_sender)).await;
            }
        }
    }
}
