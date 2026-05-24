use crate::ids::TranscriptItemId;
use crate::state::State;
use crate::transcript::TranscriptItemContent;

/// Returns whether the transcript already contains an item with the supplied ID.
pub(crate) fn transcript_contains_id(state: &State, id: &TranscriptItemId) -> bool {
    state.session.transcript.iter().any(|item| item.id == *id)
}

/// Finds the transcript index for a semantic item ID.
pub(crate) fn find_content_index_by_id(state: &State, id: &TranscriptItemId) -> Option<usize> {
    state
        .session
        .transcript
        .iter()
        .position(|item| item.id == *id)
}

/// Finds the transcript index for a tool lifecycle ID.
pub(crate) fn find_tool_call_index(state: &State, tool_call_id: &str) -> Option<usize> {
    state.session.transcript.iter().position(|item| {
        matches!(
            &item.content,
            TranscriptItemContent::ToolCall(tool_call)
                if tool_call.tool_call_id == tool_call_id
        )
    })
}

/// Finds the transcript index for a command lifecycle ID.
pub(crate) fn find_command_index(state: &State, command_id: &str) -> Option<usize> {
    state.session.transcript.iter().position(|item| {
        matches!(
            &item.content,
            TranscriptItemContent::Command(command) if command.command_id == command_id
        )
    })
}
