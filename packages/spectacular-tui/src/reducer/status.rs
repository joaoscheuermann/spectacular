use crate::action::ChatTuiAction;
use crate::reducer::transcript::{append_cancellation, append_error};
use crate::state::{default_display_context_usage, State};
use crate::status::{Activity, Status};

pub(super) fn reduce(state: &mut State, action: ChatTuiAction) {
    match action {
        ChatTuiAction::ExitRequested => {
            state.exit_requested = true;
        }
        ChatTuiAction::CancelRun => {
            if state.status.is_cancellable() {
                state.status = Status::Cancelling;
            }
        }
        ChatTuiAction::AgentStarted => {
            state.session.turn_usage = None;
            state.display.turn_usage = None;
            state.session.context_usage = None;
            state.status = Status::Running {
                activity: Activity::WaitingForModel,
                cancellable: true,
            };
        }
        ChatTuiAction::AgentFinished => {
            state.status = Status::Idle;
        }
        ChatTuiAction::AgentFailed { message, details } => {
            append_error(state, message.clone(), details);
            state.status = Status::Failed { message };
        }
        ChatTuiAction::AgentCancelled { reason } => {
            append_cancellation(state, reason);
            state.status = Status::Idle;
        }
        ChatTuiAction::InputNoticeReported { message } => {
            state.input_notice = Some(message);
        }
        ChatTuiAction::InputNoticeCleared => {
            state.input_notice = None;
        }
        ChatTuiAction::RuntimeSelectionChanged(runtime) => {
            state.runtime = runtime;
            default_display_context_usage(&state.runtime, &mut state.display);
        }
        ChatTuiAction::DisplayMetadataChanged(mut display) => {
            default_display_context_usage(&state.runtime, &mut display);
            state.display = display;
        }
        ChatTuiAction::WorktreeMetadataChanged(worktree) => {
            state.display.worktree = worktree;
        }
        ChatTuiAction::ContextUsageUpdated(usage) => {
            state.session.context_usage = Some(usage);
            state.display.context_usage = Some(usage);
        }
        ChatTuiAction::ProviderUsageReported(reported) => {
            let turn_usage = state.session.turn_usage.get_or_insert_default();
            turn_usage.record_provider_usage(reported);
            state.display.turn_usage = state.session.turn_usage;

            let total_usage = state.session.total_usage.get_or_insert_default();
            total_usage.record_provider_usage(reported);
            state.display.total_usage = state.session.total_usage;
        }
        ChatTuiAction::SpinnerTick => {
            state.spinner.tick();
        }
        _ => unreachable!("status reducer received non-status action"),
    }
}
