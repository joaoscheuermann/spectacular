use crate::action::ChatTuiAction;
use crate::ids::TranscriptItemId;
use crate::scroll::TranscriptScrollState;
use crate::session::Session;
use crate::state::State;
use crate::status::{Activity, Status};
use crate::transcript::{
    AssistantMessageItem, CancellationItem, CommandItem, CommandStatus, ErrorItem, NoticeItem,
    ReasoningItem, ToolCallItem, ToolStatus, TranscriptItem, TranscriptItemContent, UserPromptItem,
    WorkedSummaryItem,
};

/// Applies one TUI action to state without performing IO or runtime side effects.
pub fn reduce(state: &mut State, action: ChatTuiAction) {
    match action {
        ChatTuiAction::PromptChanged(prompt) => {
            state.session.prompt = prompt;
        }
        ChatTuiAction::SubmitPrompt { id, text } => {
            append_transcript_item(
                state,
                id,
                TranscriptItemContent::UserPrompt(UserPromptItem::new(text)),
            );
            state.session.prompt = crate::session::PromptState::empty();
        }
        ChatTuiAction::CancelRun => {
            if state.status.is_cancellable() {
                state.status = Status::Cancelling;
            }
        }
        ChatTuiAction::SelectionPromptChanged(selection) => {
            state.selection = selection;
        }
        ChatTuiAction::SelectionPromptSubmitted(_) | ChatTuiAction::SelectionPromptCancelled => {
            state.selection = None;
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
        ChatTuiAction::MessageStarted { id } => {
            append_transcript_item(
                state,
                id.clone(),
                TranscriptItemContent::AssistantMessage(AssistantMessageItem::new("")),
            );
            state.status = Status::Running {
                activity: Activity::StreamingAssistant { id },
                cancellable: true,
            };
        }
        ChatTuiAction::MessageDelta { id, text } => {
            append_assistant_delta(state, &id, &text);
        }
        ChatTuiAction::MessageFinished { id } => {
            clear_matching_activity(
                state,
                |activity| matches!(activity, Activity::StreamingAssistant { id: active_id } if active_id == &id),
            );
        }
        ChatTuiAction::ReasoningStarted { id } => {
            append_transcript_item(
                state,
                id.clone(),
                TranscriptItemContent::Reasoning(ReasoningItem::new("", false)),
            );
            state.status = Status::Running {
                activity: Activity::StreamingReasoning { id },
                cancellable: true,
            };
        }
        ChatTuiAction::ReasoningDelta { id, text } => {
            append_reasoning_delta(state, &id, &text);
        }
        ChatTuiAction::ReasoningFinished { id } => {
            clear_matching_activity(
                state,
                |activity| matches!(activity, Activity::StreamingReasoning { id: active_id } if active_id == &id),
            );
        }
        ChatTuiAction::ToolCallStarted {
            id,
            tool_call_id,
            name,
            arguments,
        } => {
            let arguments_preview = optional_preview(arguments);
            append_transcript_item(
                state,
                id.clone(),
                TranscriptItemContent::ToolCall(ToolCallItem::running(
                    tool_call_id,
                    name.clone(),
                    arguments_preview,
                )),
            );
            state.status = Status::Running {
                activity: Activity::RunningTool { id, name },
                cancellable: true,
            };
        }
        ChatTuiAction::ToolCallDelta { tool_call_id, text } => {
            append_tool_delta(state, &tool_call_id, &text);
        }
        ChatTuiAction::ToolCallFinished {
            tool_call_id,
            name,
            output,
        } => {
            let active_item_id = matching_tool_activity_item_id(state, &tool_call_id);
            finish_tool_call(state, &tool_call_id, name, output);
            clear_matching_activity(
                state,
                |activity| matches!(activity, Activity::RunningTool { id, .. } if Some(id) == active_item_id.as_ref()),
            );
        }
        ChatTuiAction::CommandStarted {
            id,
            command_id,
            command,
        } => {
            append_transcript_item(
                state,
                id.clone(),
                TranscriptItemContent::Command(CommandItem::running(command_id.clone(), command)),
            );
            state.status = Status::Running {
                activity: Activity::RunningCommand { id, command_id },
                cancellable: true,
            };
        }
        ChatTuiAction::CommandOutput { command_id, text } => {
            append_command_output(state, &command_id, &text);
        }
        ChatTuiAction::CommandFinished {
            command_id,
            exit_code,
        } => {
            finish_command(state, &command_id, exit_code);
            clear_matching_activity(
                state,
                |activity| matches!(activity, Activity::RunningCommand { command_id: active_id, .. } if active_id == &command_id),
            );
        }
        ChatTuiAction::AgentFinished => {
            state.status = Status::Idle;
        }
        ChatTuiAction::WorkedSummaryReported {
            duration,
            turn_tokens,
        } => {
            append_worked_summary(state, duration, turn_tokens);
        }
        ChatTuiAction::AgentFailed { message } => {
            append_error(state, message.clone(), None);
            state.status = Status::Failed { message };
        }
        ChatTuiAction::AgentCancelled { reason } => {
            append_cancellation(state, reason);
            state.status = Status::Idle;
        }
        ChatTuiAction::ErrorReported { message, details } => {
            append_error(state, message, details);
        }
        ChatTuiAction::NoticeReported { message } => {
            append_notice(state, message);
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
            clamp_scroll_to_transcript(&mut state.scroll, state.session.transcript.len());
        }
        ChatTuiAction::Resize { height, .. } => {
            state.scroll.visible_rows = u32::from(height);
            clamp_scroll_to_transcript(&mut state.scroll, state.session.transcript.len());
        }
    }
}

/// Clamps transcript scroll offset to the valid range for the current transcript length.
fn clamp_scroll_to_transcript(scroll: &mut TranscriptScrollState, transcript_len: usize) {
    if scroll.visible_rows == 0 {
        return;
    }

    let max_offset = (transcript_len as u32).saturating_sub(scroll.visible_rows);
    scroll.offset = scroll.offset.min(max_offset);
    scroll.follow_tail = scroll.offset == 0;
}

/// Appends a semantic transcript item with the next session timestamp.
fn append_transcript_item(state: &mut State, id: TranscriptItemId, content: TranscriptItemContent) {
    preserve_review_position_for_append(state);
    let timestamp = state.session.allocate_timestamp();
    state
        .session
        .transcript
        .push(TranscriptItem::new(id, timestamp, content));
}

/// Keeps the same rendered rows visible when appending while not following the transcript tail.
fn preserve_review_position_for_append(state: &mut State) {
    if state.scroll.follow_tail {
        return;
    }

    state.scroll.offset = state.scroll.offset.saturating_add(1);
    clamp_scroll_to_transcript(
        &mut state.scroll,
        state.session.transcript.len().saturating_add(1),
    );
}

/// Appends text to an assistant message matching the supplied transcript item ID.
fn append_assistant_delta(state: &mut State, id: &TranscriptItemId, text: &str) {
    let Some(TranscriptItemContent::AssistantMessage(item)) = find_content_by_id(state, id) else {
        return;
    };

    item.text.push_str(text);
}

/// Appends text to reasoning content matching the supplied transcript item ID.
fn append_reasoning_delta(state: &mut State, id: &TranscriptItemId, text: &str) {
    let Some(TranscriptItemContent::Reasoning(item)) = find_content_by_id(state, id) else {
        return;
    };

    item.text.push_str(text);
}

/// Appends incremental output preview text to a tool call by lifecycle ID.
fn append_tool_delta(state: &mut State, tool_call_id: &str, text: &str) {
    let Some(tool_call) = find_tool_call(state, tool_call_id) else {
        return;
    };

    let output = tool_call.output_preview.get_or_insert_with(String::new);
    output.push_str(text);
}

/// Marks a tool call finished while preserving its start-time identity metadata.
fn finish_tool_call(state: &mut State, tool_call_id: &str, _name: String, output: String) {
    let Some(tool_call) = find_tool_call(state, tool_call_id) else {
        return;
    };

    tool_call.status = ToolStatus::Finished;
    tool_call.output_preview = Some(output);
}

/// Appends command output to the matching command transcript item.
fn append_command_output(state: &mut State, command_id: &str, text: &str) {
    let Some(command) = find_command(state, command_id) else {
        return;
    };

    command.output.push_str(text);
}

/// Marks a command complete and records its exit code.
fn finish_command(state: &mut State, command_id: &str, exit_code: Option<i32>) {
    let Some(command) = find_command(state, command_id) else {
        return;
    };

    command.status = match exit_code {
        Some(0) | None => CommandStatus::Finished,
        Some(_) => CommandStatus::Failed,
    };
    command.exit_code = exit_code;
}

/// Appends a semantic error transcript item using a reducer-owned ID.
fn append_error(state: &mut State, message: String, details: Option<String>) {
    let id = generated_transcript_id(state, "error");
    append_transcript_item(
        state,
        id,
        TranscriptItemContent::Error(ErrorItem::new(message, details)),
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

/// Appends a semantic cancellation transcript item using a reducer-owned ID.
fn append_cancellation(state: &mut State, reason: String) {
    let id = generated_transcript_id(state, "cancellation");
    append_transcript_item(
        state,
        id,
        TranscriptItemContent::Cancellation(CancellationItem::new(reason)),
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

/// Returns a non-empty preview for optional argument storage.
fn optional_preview(value: String) -> Option<String> {
    if value.is_empty() {
        return None;
    }

    Some(value)
}

/// Clears the active running status if the supplied predicate matches its activity.
fn clear_matching_activity(state: &mut State, matches_activity: impl FnOnce(&Activity) -> bool) {
    let Status::Running { activity, .. } = &state.status else {
        return;
    };

    if !matches_activity(activity) {
        return;
    }

    state.status = Status::Idle;
}

/// Finds mutable semantic content by transcript item ID.
fn find_content_by_id<'a>(
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
fn find_tool_call<'a>(state: &'a mut State, tool_call_id: &str) -> Option<&'a mut ToolCallItem> {
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
fn find_command<'a>(state: &'a mut State, command_id: &str) -> Option<&'a mut CommandItem> {
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
fn matching_tool_activity_item_id(state: &State, tool_call_id: &str) -> Option<TranscriptItemId> {
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
