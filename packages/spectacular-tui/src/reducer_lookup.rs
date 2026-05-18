use crate::ids::TranscriptItemId;
use crate::state::State;
use crate::status::{Activity, Status};
use crate::transcript::{CommandItem, ToolCallItem, TranscriptItemContent};

/// Clears the active running status if the supplied predicate matches its activity.
pub(crate) fn clear_matching_activity(
    state: &mut State,
    matches_activity: impl FnOnce(&Activity) -> bool,
) {
    let Status::Running { activity, .. } = &state.status else {
        return;
    };

    if !matches_activity(activity) {
        return;
    }

    state.status = Status::Idle;
}

/// Returns whether the transcript already contains an item with the supplied ID.
pub(crate) fn transcript_contains_id(state: &State, id: &TranscriptItemId) -> bool {
    state.session.transcript.iter().any(|item| item.id == *id)
}

/// Finds mutable semantic content by transcript item ID.
pub(crate) fn find_content_by_id<'a>(
    state: &'a mut State,
    id: &TranscriptItemId,
) -> Option<&'a mut TranscriptItemContent> {
    state
        .session
        .transcript
        .iter_mut()
        .find(|item| item.id == *id)
        .map(|item| &mut item.content)
}

/// Finds a mutable tool-call transcript item by tool lifecycle ID.
pub(crate) fn find_tool_call<'a>(
    state: &'a mut State,
    tool_call_id: &str,
) -> Option<&'a mut ToolCallItem> {
    state
        .session
        .transcript
        .iter_mut()
        .find_map(|item| match &mut item.content {
            TranscriptItemContent::ToolCall(tool_call)
                if tool_call.tool_call_id == tool_call_id =>
            {
                Some(tool_call)
            }
            _ => None,
        })
}

/// Finds a mutable command transcript item by command lifecycle ID.
pub(crate) fn find_command<'a>(
    state: &'a mut State,
    command_id: &str,
) -> Option<&'a mut CommandItem> {
    state
        .session
        .transcript
        .iter_mut()
        .find_map(|item| match &mut item.content {
            TranscriptItemContent::Command(command) if command.command_id == command_id => {
                Some(command)
            }
            _ => None,
        })
}

/// Returns the active running tool item ID when it matches the supplied lifecycle ID.
pub(crate) fn matching_tool_activity_item_id(
    state: &State,
    tool_call_id: &str,
) -> Option<TranscriptItemId> {
    let Status::Running {
        activity: Activity::RunningTool { id, .. },
        ..
    } = &state.status
    else {
        return None;
    };

    if !transcript_item_has_tool_call(state, id, tool_call_id) {
        return None;
    }

    Some(id.clone())
}

/// Returns whether a transcript item ID points at the supplied tool lifecycle ID.
fn transcript_item_has_tool_call(state: &State, id: &TranscriptItemId, tool_call_id: &str) -> bool {
    state.session.transcript.iter().any(|item| {
        item.id == *id
            && matches!(
                &item.content,
                TranscriptItemContent::ToolCall(tool_call)
                    if tool_call.tool_call_id == tool_call_id
            )
    })
}
