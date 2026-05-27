use crate::action::ChatTuiAction;
use crate::ids::TranscriptItemId;
use crate::reducer::lookup::find_content_index_by_id;
use crate::state::State;
use crate::transcript::{
    AssistantMessageItem, CancellationItem, ErrorItem, NoticeItem, ReasoningItem, SuccessItem,
    TranscriptItem, TranscriptItemContent, WarningItem, WorkedSummaryItem,
};

pub(super) fn reduce(state: &mut State, action: ChatTuiAction) {
    match action {
        ChatTuiAction::MessageStarted { id } => {
            append_transcript_item(
                state,
                id,
                TranscriptItemContent::AssistantMessage(AssistantMessageItem::new("")),
            );
        }
        ChatTuiAction::MessageDelta { id, text } => {
            append_assistant_delta_directly(state, &id, &text);
        }
        ChatTuiAction::MessageFinished { id: _ } => {}
        ChatTuiAction::ReasoningStarted { id } => {
            append_transcript_item(
                state,
                id,
                TranscriptItemContent::Reasoning(ReasoningItem::new("", false)),
            );
        }
        ChatTuiAction::ReasoningDelta { id, text } => {
            append_reasoning_delta(state, &id, &text);
        }
        ChatTuiAction::ReasoningFinished { id: _ } => {}
        ChatTuiAction::WorkedSummaryReported {
            duration,
            turn_tokens,
        } => {
            append_worked_summary(state, duration, turn_tokens);
        }
        ChatTuiAction::ErrorReported { message, details } => {
            append_error(state, message, details);
        }
        ChatTuiAction::WarningReported { message } => {
            append_warning(state, message);
        }
        ChatTuiAction::SuccessReported { message } => {
            append_success(state, message);
        }
        ChatTuiAction::NoticeReported { message } => {
            append_notice(state, message);
        }
        _ => unreachable!("transcript reducer received non-transcript action"),
    }
}

/// Appends a semantic transcript item with the next session timestamp.
pub(super) fn append_transcript_item(
    state: &mut State,
    id: TranscriptItemId,
    content: TranscriptItemContent,
) {
    let timestamp = state.session.allocate_timestamp();
    state
        .session
        .transcript
        .push(TranscriptItem::new(id, timestamp, content));
}

/// Appends a semantic error transcript item using a reducer-owned ID.
pub(super) fn append_error(state: &mut State, message: String, details: Option<String>) {
    let id = generated_transcript_id(state, "error");
    append_transcript_item(
        state,
        id,
        TranscriptItemContent::Error(ErrorItem::new(message, details)),
    );
}

/// Appends a semantic cancellation transcript item using a reducer-owned ID.
pub(super) fn append_cancellation(state: &mut State, reason: String) {
    let id = generated_transcript_id(state, "cancellation");
    append_transcript_item(
        state,
        id,
        TranscriptItemContent::Cancellation(CancellationItem::new(reason)),
    );
}

/// Appends assistant text directly to the semantic transcript item.
fn append_assistant_delta_directly(state: &mut State, id: &TranscriptItemId, text: &str) {
    let Some(index) = find_content_index_by_id(state, id) else {
        return;
    };
    let TranscriptItemContent::AssistantMessage(item) =
        &mut state.session.transcript[index].content
    else {
        return;
    };

    item.text.push_str(text);
}

/// Appends text to reasoning content matching the supplied transcript item ID.
fn append_reasoning_delta(state: &mut State, id: &TranscriptItemId, text: &str) {
    let Some(index) = find_content_index_by_id(state, id) else {
        return;
    };
    let TranscriptItemContent::Reasoning(item) = &mut state.session.transcript[index].content
    else {
        return;
    };

    item.text.push_str(text);
}

/// Appends a semantic warning transcript item using a reducer-owned ID.
fn append_warning(state: &mut State, message: String) {
    let id = generated_transcript_id(state, "warning");
    append_transcript_item(
        state,
        id,
        TranscriptItemContent::Warning(WarningItem::new(message)),
    );
}

/// Appends a semantic success transcript item using a reducer-owned ID.
fn append_success(state: &mut State, message: String) {
    let id = generated_transcript_id(state, "success");
    append_transcript_item(
        state,
        id,
        TranscriptItemContent::Success(SuccessItem::new(message)),
    );
}

/// Appends a semantic notice transcript item using a reducer-owned ID.
fn append_notice(state: &mut State, message: String) {
    let id = generated_transcript_id(state, "notice");
    append_transcript_item(
        state,
        id,
        TranscriptItemContent::Notice(NoticeItem::new(message)),
    );
}

/// Appends a worked-summary transcript item using a reducer-owned ID.
fn append_worked_summary(state: &mut State, duration: String, turn_tokens: Option<u64>) {
    let id = generated_transcript_id(state, "worked-summary");
    append_transcript_item(
        state,
        id,
        TranscriptItemContent::WorkedSummary(WorkedSummaryItem::new(duration, turn_tokens)),
    );
}

/// Generates deterministic reducer-boundary IDs for reducer-created transcript items.
fn generated_transcript_id(state: &State, prefix: &str) -> TranscriptItemId {
    TranscriptItemId::new(format!("{prefix}-{}", state.session.next_timestamp.value()))
}
