use super::controller::{Bootstrap, Controller};
use super::runner::TurnRunner;
use crate::chat::{ChatBootstrap, ChatError};
use ::llms::LlmDebugLogger;
use ::tui::{root_element, Intent, State};
use iocraft::prelude::*;
use std::future::Future;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::{JoinError, JoinHandle};

type RenderTask = JoinHandle<io::Result<()>>;

struct RenderChannels {
    intent_sender: mpsc::UnboundedSender<Intent>,
    cancellation_sender: mpsc::UnboundedSender<()>,
    selection_sender: mpsc::UnboundedSender<Intent>,
    state_receiver: Arc<Mutex<mpsc::UnboundedReceiver<State>>>,
}

struct ControllerChannels {
    intent_receiver: mpsc::UnboundedReceiver<Intent>,
    cancellation_receiver: mpsc::UnboundedReceiver<()>,
    selection_receiver: mpsc::UnboundedReceiver<Intent>,
    state_sender: mpsc::UnboundedSender<State>,
}

enum LoopEvent {
    Intent(Option<Intent>),
    WorktreeRefresh,
}

/// Runs the production IOCraft render loop with the real Doric runtime controller.
pub(crate) async fn run(debug_logger: LlmDebugLogger) -> Result<String, ChatError> {
    let bootstrap = Bootstrap::from_chat_bootstrap(ChatBootstrap::new(debug_logger)?)?;
    run_with_controller(Controller::new(bootstrap)?).await
}

/// Coordinates IOCraft-rendered terminal UI with controller-owned async runtime work.
async fn run_with_controller<R>(controller: Controller<R>) -> Result<String, ChatError>
where
    R: TurnRunner + 'static,
{
    let initial_state = controller.state_snapshot();
    let (render_channels, controller_channels) = runtime_channels();
    let render_task = spawn_render_task(initial_state, render_channels);
    let ControllerChannels {
        intent_receiver,
        cancellation_receiver,
        selection_receiver,
        state_sender,
    } = controller_channels;
    let controller_loop = run_controller_loop(
        controller,
        intent_receiver,
        cancellation_receiver,
        selection_receiver,
        state_sender,
    );
    join_runtime_tasks(render_task, controller_loop).await
}

fn runtime_channels() -> (RenderChannels, ControllerChannels) {
    let (intent_sender, intent_receiver) = mpsc::unbounded_channel();
    let (cancellation_sender, cancellation_receiver) = mpsc::unbounded_channel();
    let (selection_sender, selection_receiver) = mpsc::unbounded_channel();
    let (state_sender, state_receiver) = mpsc::unbounded_channel();

    (
        RenderChannels {
            intent_sender,
            cancellation_sender,
            selection_sender,
            state_receiver: Arc::new(Mutex::new(state_receiver)),
        },
        ControllerChannels {
            intent_receiver,
            cancellation_receiver,
            selection_receiver,
            state_sender,
        },
    )
}

fn spawn_render_task(initial_state: State, channels: RenderChannels) -> RenderTask {
    let runtime = tokio::runtime::Handle::current();
    tokio::task::spawn_blocking(move || {
        runtime.block_on(async move {
            root_element(
                initial_state,
                channels.intent_sender,
                channels.cancellation_sender,
                channels.selection_sender,
                channels.state_receiver,
            )
            .fullscreen()
            .ignore_ctrl_c()
            .await
        })
    })
}

async fn join_runtime_tasks(
    render_task: RenderTask,
    controller_loop: impl Future<Output = Result<String, ChatError>>,
) -> Result<String, ChatError> {
    let (render_result, controller_result) = tokio::join!(render_task, controller_loop);
    finish_render_task(render_result)?;
    controller_result
}

fn finish_render_task(result: Result<io::Result<()>, JoinError>) -> Result<(), ChatError> {
    let render_result = result.map_err(|error| ChatError::Session(error.to_string()))?;
    render_result.map_err(ChatError::Io)
}

/// Processes intents emitted by the IOCraft shell and publishes reducer snapshots for rendering.
pub(crate) async fn run_controller_loop<R>(
    controller: Controller<R>,
    intent_receiver: mpsc::UnboundedReceiver<Intent>,
    cancellation_receiver: mpsc::UnboundedReceiver<()>,
    selection_receiver: mpsc::UnboundedReceiver<Intent>,
    state_sender: mpsc::UnboundedSender<State>,
) -> Result<String, ChatError>
where
    R: TurnRunner,
{
    ControllerChannels {
        intent_receiver,
        cancellation_receiver,
        selection_receiver,
        state_sender,
    }
    .run(controller)
    .await
}

impl ControllerChannels {
    async fn run<R>(mut self, mut controller: Controller<R>) -> Result<String, ChatError>
    where
        R: TurnRunner,
    {
        let mut worktree_refresh = tokio::time::interval(Duration::from_secs(2));

        loop {
            match next_loop_event(&mut self.intent_receiver, &mut worktree_refresh).await {
                LoopEvent::Intent(Some(intent)) => {
                    if self.handle_intent(&mut controller, intent).await? {
                        return Ok(controller.current_session_id().to_owned());
                    }
                }
                LoopEvent::Intent(None) => return Ok(controller.current_session_id().to_owned()),
                LoopEvent::WorktreeRefresh => {
                    controller
                        .refresh_worktree_metadata(Some(&self.state_sender))
                        .await;
                }
            }
        }
    }

    async fn handle_intent<R>(
        &mut self,
        controller: &mut Controller<R>,
        intent: Intent,
    ) -> Result<bool, ChatError>
    where
        R: TurnRunner,
    {
        let should_exit = controller
            .handle_intent_with_state_sender(
                intent,
                &self.state_sender,
                &mut self.cancellation_receiver,
                &mut self.selection_receiver,
            )
            .await?;
        let _ = self.state_sender.send(controller.state_snapshot());
        Ok(should_exit)
    }
}

async fn next_loop_event(
    intent_receiver: &mut mpsc::UnboundedReceiver<Intent>,
    worktree_refresh: &mut tokio::time::Interval,
) -> LoopEvent {
    tokio::select! {
        intent = intent_receiver.recv() => LoopEvent::Intent(intent),
        _ = worktree_refresh.tick() => LoopEvent::WorktreeRefresh,
    }
}
