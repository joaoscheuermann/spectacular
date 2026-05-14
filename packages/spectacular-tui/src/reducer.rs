use crate::action::ChatTuiAction;
use crate::session::Session;
use crate::state::State;
use crate::status::{Activity, Status};

/// Applies one TUI action to state without performing IO or runtime side effects.
pub fn reduce(state: &mut State, action: ChatTuiAction) {
    match action {
        ChatTuiAction::PromptChanged(prompt) => {
            state.session.prompt = prompt;
        }
        ChatTuiAction::SubmitPrompt(_) => {
            state.session.prompt.clear();
        }
        ChatTuiAction::CancelRun => {
            if state.status.is_cancellable() {
                state.status = Status::Cancelling;
            }
        }
        ChatTuiAction::CommandsLoaded(commands) => {
            state.commands = commands;
        }
        ChatTuiAction::SessionChanged { id } => {
            state.session = Session::new(id);
            state.scroll = Default::default();
        }
        ChatTuiAction::AgentStarted => {
            state.status = Status::Running {
                activity: Activity::WaitingForModel,
                cancellable: true,
            };
        }
        ChatTuiAction::AgentFinished => {
            state.status = Status::Idle;
        }
        ChatTuiAction::AgentFailed { message } => {
            state.status = Status::Failed { message };
        }
        ChatTuiAction::AgentCancelled { reason } => {
            state.status = Status::Failed { message: reason };
        }
        ChatTuiAction::RuntimeSelectionChanged(runtime) => {
            state.runtime = runtime;
        }
        ChatTuiAction::DisplayMetadataChanged(display) => {
            state.display = display;
        }
        ChatTuiAction::UsageUpdated(usage) => {
            state.session.usage = Some(usage);
            state.display.usage = Some(usage);
        }
        ChatTuiAction::SpinnerTick => {
            state.spinner.tick();
        }
        ChatTuiAction::ScrollTranscript(delta) => {
            state.scroll.scroll_by(delta);
        }
        ChatTuiAction::Resize => {}
    }
}
