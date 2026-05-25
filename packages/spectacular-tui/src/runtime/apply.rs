use crate::action::ChatTuiAction;
use crate::ids::TranscriptItemId;
use crate::reducer::reduce;
use crate::runtime::{EventEffect, Intent};
use crate::state::State;
use crate::transcript::TranscriptItemContent;
use crate::view::{
    apply_view_action, clear_selection, preserve_review_position_for_layout_change,
    transcript_layout_snapshot, ViewState,
};

/// Applies local event effects to semantic state and view state, returning controller intents.
pub(crate) fn apply_event_effects(
    state: &mut State,
    view: &mut ViewState,
    effects: Vec<EventEffect>,
) -> Vec<Intent> {
    let mut intents = Vec::new();
    for effect in effects {
        match effect {
            EventEffect::Action(action) => {
                if let Some(intent) = apply_user_action(state, view, *action) {
                    intents.push(intent);
                }
            }
            EventEffect::ViewAction(action) => {
                apply_view_action(view, state, *action);
            }
            EventEffect::RequestExit => intents.push(Intent::RequestExit),
        }
    }

    intents
}

/// Applies a controller-originated reducer action without emitting a user intent.
pub(crate) fn apply_controller_action(
    state: &mut State,
    view: &mut ViewState,
    action: ChatTuiAction,
) {
    let _ = apply_action(state, view, action);
}

/// Records transcript layout invalidation for a controller-published state snapshot.
pub(crate) fn record_controller_transcript_update(
    view: &mut ViewState,
    previous: &State,
    next: &State,
) {
    if previous.session.id != next.session.id {
        view.reset_for_session();
        return;
    }

    if let Some(start_index) = changed_suffix_start(previous, next) {
        view.note_transcript_change(start_index);
    }
}

fn apply_user_action(
    state: &mut State,
    view: &mut ViewState,
    action: ChatTuiAction,
) -> Option<Intent> {
    apply_action(state, view, action)
}

fn apply_action(state: &mut State, view: &mut ViewState, action: ChatTuiAction) -> Option<Intent> {
    let intent = intent_for_action(&action);
    let should_clear_selection = clears_rendered_selection(&action);
    let transcript_update = transcript_update_for_action(state, &action);
    let old_layout = transcript_layout_snapshot(state, view).layout;

    reduce(state, action);

    match transcript_update {
        TranscriptUpdate::Reset => view.reset_for_session(),
        TranscriptUpdate::Changed(start_index) => view.note_transcript_change(start_index),
        TranscriptUpdate::Unchanged => {}
    }

    if transcript_update != TranscriptUpdate::Reset {
        let new_layout = transcript_layout_snapshot(state, view).layout;
        preserve_review_position_for_layout_change(view, &old_layout, &new_layout);
        if should_clear_selection {
            clear_selection(view);
        }
    }

    intent
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TranscriptUpdate {
    Unchanged,
    Changed(usize),
    Reset,
}

fn transcript_update_for_action(state: &State, action: &ChatTuiAction) -> TranscriptUpdate {
    match action {
        ChatTuiAction::SessionChanged { .. } | ChatTuiAction::SessionCreated { .. } => {
            TranscriptUpdate::Reset
        }
        ChatTuiAction::SubmitPrompt { id, .. } => submit_prompt_update(state, id),
        ChatTuiAction::MessageStarted { .. }
        | ChatTuiAction::ReasoningStarted { .. }
        | ChatTuiAction::ToolCallStarted { .. }
        | ChatTuiAction::CommandStarted { .. }
        | ChatTuiAction::WorkedSummaryReported { .. }
        | ChatTuiAction::AgentFailed { .. }
        | ChatTuiAction::AgentCancelled { .. }
        | ChatTuiAction::ErrorReported { .. }
        | ChatTuiAction::WarningReported { .. }
        | ChatTuiAction::SuccessReported { .. }
        | ChatTuiAction::NoticeReported { .. } => {
            TranscriptUpdate::Changed(state.session.transcript.len())
        }
        ChatTuiAction::MessageDelta { id, .. } | ChatTuiAction::ReasoningDelta { id, .. } => {
            content_index_by_id(state, id)
                .map(TranscriptUpdate::Changed)
                .unwrap_or(TranscriptUpdate::Unchanged)
        }
        ChatTuiAction::ToolCallDelta { tool_call_id, .. }
        | ChatTuiAction::ToolCallFinished { tool_call_id, .. }
        | ChatTuiAction::ToolCallFailed { tool_call_id, .. }
        | ChatTuiAction::ToolDisplayFinished { tool_call_id, .. } => {
            tool_call_index(state, tool_call_id)
                .map(TranscriptUpdate::Changed)
                .unwrap_or(TranscriptUpdate::Unchanged)
        }
        ChatTuiAction::ToolDisplayStarted { tool_call_id, .. } => {
            tool_call_index(state, tool_call_id)
                .map(TranscriptUpdate::Changed)
                .unwrap_or_else(|| TranscriptUpdate::Changed(state.session.transcript.len()))
        }
        ChatTuiAction::CommandOutput { command_id, .. }
        | ChatTuiAction::CommandFinished { command_id, .. }
        | ChatTuiAction::CommandDisplayOutput { command_id, .. }
        | ChatTuiAction::CommandDisplayFinished { command_id, .. } => {
            command_index(state, command_id)
                .map(TranscriptUpdate::Changed)
                .unwrap_or(TranscriptUpdate::Unchanged)
        }
        ChatTuiAction::CommandDisplayStarted { command_id, .. } => command_index(state, command_id)
            .map(TranscriptUpdate::Changed)
            .unwrap_or_else(|| TranscriptUpdate::Changed(state.session.transcript.len())),
        _ => TranscriptUpdate::Unchanged,
    }
}

fn submit_prompt_update(state: &State, id: &TranscriptItemId) -> TranscriptUpdate {
    if let Some(index) = content_index_by_id(state, id) {
        if matches!(
            state.session.transcript[index].content,
            TranscriptItemContent::UserPrompt(_)
        ) {
            return TranscriptUpdate::Changed(index);
        }

        return TranscriptUpdate::Unchanged;
    }

    TranscriptUpdate::Changed(state.session.transcript.len())
}

fn changed_suffix_start(previous: &State, next: &State) -> Option<usize> {
    let previous_len = previous.session.transcript.len();
    let next_len = next.session.transcript.len();
    if previous_len != next_len {
        return Some(previous_len.min(next_len));
    }

    previous
        .session
        .transcript
        .iter()
        .zip(next.session.transcript.iter())
        .enumerate()
        .rev()
        .find_map(|(index, (previous, next))| (previous != next).then_some(index))
}

fn content_index_by_id(state: &State, id: &TranscriptItemId) -> Option<usize> {
    state
        .session
        .transcript
        .iter()
        .position(|item| item.id == *id)
}

fn tool_call_index(state: &State, tool_call_id: &str) -> Option<usize> {
    state
        .session
        .transcript
        .iter()
        .position(|item| match &item.content {
            TranscriptItemContent::ToolCall(tool) => tool.tool_call_id == tool_call_id,
            _ => false,
        })
}

fn command_index(state: &State, command_id: &str) -> Option<usize> {
    state
        .session
        .transcript
        .iter()
        .position(|item| match &item.content {
            TranscriptItemContent::Command(command) => command.command_id == command_id,
            _ => false,
        })
}
