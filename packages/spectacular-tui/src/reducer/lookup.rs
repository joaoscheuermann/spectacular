use crate::ids::TranscriptItemId;
use crate::state::State;
use crate::transcript::{CommandItem, ToolCallItem, TranscriptItemContent};

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
