use spectacular_tui::{
    reduce, Activity, ChatTuiAction, CommandStatus, DisplayMetadata, PromptState, ReasoningLevel,
    RuntimeSelection, SessionId, State, Status, ToolStatus, TranscriptItemContent,
    TranscriptItemId,
};

/// Builds a representative runtime selection for transcript reducer tests.
fn runtime() -> RuntimeSelection {
    RuntimeSelection::new(
        "openai-compatible",
        "provider",
        "model",
        ReasoningLevel::Low,
        Some(4096),
    )
}

/// Builds visible display metadata for transcript reducer tests.
fn display() -> DisplayMetadata {
    DisplayMetadata::new("provider", "model", "low", "/workspace", "session", None)
}

/// Builds an initialized state with stable metadata.
fn state() -> State {
    State::new(SessionId::new("session-1"), runtime(), display())
}

/// Builds a stable transcript item ID for lifecycle tests.
fn item_id(value: &str) -> TranscriptItemId {
    TranscriptItemId::new(value)
}

/// Verifies assistant lifecycle actions update one semantic assistant item and status.
#[test]
fn assistant_lifecycle_updates_one_item_and_status() {
    let mut state = state();
    let id = item_id("assistant-1");

    reduce(&mut state, ChatTuiAction::MessageStarted { id: id.clone() });
    reduce(
        &mut state,
        ChatTuiAction::MessageDelta {
            id: id.clone(),
            text: "hello ".to_owned(),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::MessageDelta {
            id: id.clone(),
            text: "world".to_owned(),
        },
    );

    assert_eq!(state.session.transcript.len(), 1);
    assert_eq!(
        state.status,
        Status::Running {
            activity: Activity::StreamingAssistant { id: id.clone() },
            cancellable: true,
        }
    );
    assert!(matches!(
        &state.session.transcript[0].content,
        TranscriptItemContent::AssistantMessage(item) if item.text == "hello world"
    ));

    reduce(&mut state, ChatTuiAction::MessageFinished { id });

    assert_eq!(state.status, Status::Idle);
    assert_eq!(state.session.transcript.len(), 1);
}

/// Verifies reasoning lifecycle actions update one reasoning item and status.
#[test]
fn reasoning_lifecycle_updates_one_item_and_status() {
    let mut state = state();
    let id = item_id("reasoning-1");

    reduce(
        &mut state,
        ChatTuiAction::ReasoningStarted { id: id.clone() },
    );
    reduce(
        &mut state,
        ChatTuiAction::ReasoningDelta {
            id: id.clone(),
            text: "step 1. ".to_owned(),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::ReasoningDelta {
            id: id.clone(),
            text: "step 2.".to_owned(),
        },
    );

    assert_eq!(state.session.transcript.len(), 1);
    assert_eq!(
        state.status,
        Status::Running {
            activity: Activity::StreamingReasoning { id: id.clone() },
            cancellable: true,
        }
    );
    assert!(matches!(
        &state.session.transcript[0].content,
        TranscriptItemContent::Reasoning(item) if item.text == "step 1. step 2." && !item.collapsed
    ));

    reduce(&mut state, ChatTuiAction::ReasoningFinished { id });

    assert_eq!(state.status, Status::Idle);
    assert_eq!(state.session.transcript.len(), 1);
}

/// Verifies tool lifecycle actions update one semantic tool item without raw output items.
#[test]
fn tool_lifecycle_updates_one_item_and_preserves_identity() {
    let mut state = state();
    let id = item_id("tool-item-1");

    reduce(
        &mut state,
        ChatTuiAction::ToolCallStarted {
            id: id.clone(),
            tool_call_id: "call-1".to_owned(),
            name: "grep".to_owned(),
            arguments: "pattern: State".to_owned(),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::ToolCallDelta {
            tool_call_id: "call-1".to_owned(),
            text: "partial".to_owned(),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::ToolCallFinished {
            tool_call_id: "call-1".to_owned(),
            name: "unexpected-finish-name".to_owned(),
            output: "final output".to_owned(),
        },
    );

    assert_eq!(state.status, Status::Idle);
    assert_eq!(state.session.transcript.len(), 1);
    assert_eq!(state.session.transcript[0].id, id);
    assert!(matches!(
        &state.session.transcript[0].content,
        TranscriptItemContent::ToolCall(item)
            if item.tool_call_id == "call-1"
                && item.name == "grep"
                && item.arguments_preview.as_deref() == Some("pattern: State")
                && item.status == ToolStatus::Finished
                && item.output_preview.as_deref() == Some("final output")
    ));
}

/// Verifies command lifecycle actions update one command item and record exit code.
#[test]
fn command_lifecycle_updates_one_item_and_exit_code() {
    let mut state = state();
    let id = item_id("command-item-1");

    reduce(
        &mut state,
        ChatTuiAction::CommandStarted {
            id: id.clone(),
            command_id: "cmd-1".to_owned(),
            command: "cargo test".to_owned(),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::CommandOutput {
            command_id: "cmd-1".to_owned(),
            text: "running\n".to_owned(),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::CommandFinished {
            command_id: "cmd-1".to_owned(),
            exit_code: Some(0),
        },
    );

    assert_eq!(state.status, Status::Idle);
    assert_eq!(state.session.transcript.len(), 1);
    assert_eq!(state.session.transcript[0].id, id);
    assert!(matches!(
        &state.session.transcript[0].content,
        TranscriptItemContent::Command(item)
            if item.command_id == "cmd-1"
                && item.command == "cargo test"
                && item.output == "running\n"
                && item.status == CommandStatus::Finished
                && item.exit_code == Some(0)
    ));
}

/// Verifies failed command exits are represented on the existing command item.
#[test]
fn command_failed_exit_marks_existing_item_failed() {
    let mut state = state();

    reduce(
        &mut state,
        ChatTuiAction::CommandStarted {
            id: item_id("command-item-1"),
            command_id: "cmd-1".to_owned(),
            command: "false".to_owned(),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::CommandFinished {
            command_id: "cmd-1".to_owned(),
            exit_code: Some(1),
        },
    );

    assert!(matches!(
        &state.session.transcript[0].content,
        TranscriptItemContent::Command(item)
            if item.status == CommandStatus::Failed && item.exit_code == Some(1)
    ));
}

/// Verifies error, cancellation, and notice actions append semantic transcript items.
#[test]
fn error_cancellation_and_notice_append_semantic_items() {
    let mut state = state();
    reduce(
        &mut state,
        ChatTuiAction::ErrorReported {
            message: "bad".to_owned(),
            details: Some("stack".to_owned()),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::AgentCancelled {
            reason: "cancelled".to_owned(),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::NoticeReported {
            message: "heads up".to_owned(),
        },
    );

    assert_eq!(state.status, Status::Idle);
    assert_eq!(state.session.transcript.len(), 3);
    assert!(matches!(
        &state.session.transcript[0].content,
        TranscriptItemContent::Error(item)
            if item.message == "bad" && item.details.as_deref() == Some("stack")
    ));
    assert!(matches!(
        &state.session.transcript[1].content,
        TranscriptItemContent::Notice(item) if item.message == "cancelled"
    ));
    assert!(matches!(
        &state.session.transcript[2].content,
        TranscriptItemContent::Notice(item) if item.message == "heads up"
    ));
}

/// Verifies appending transcript content does not yank a non-following viewport.
#[test]
fn appended_content_honors_scroll_follow_tail_state() {
    let mut following = state();
    reduce(
        &mut following,
        ChatTuiAction::SubmitPrompt {
            id: item_id("prompt-1"),
            text: "hello".to_owned(),
        },
    );
    assert_eq!(following.scroll.offset, 0);
    assert!(following.scroll.follow_tail);

    let mut reviewing = state();
    reduce(&mut reviewing, ChatTuiAction::ScrollTranscript(4));
    reduce(
        &mut reviewing,
        ChatTuiAction::MessageStarted {
            id: item_id("assistant-1"),
        },
    );

    assert_eq!(reviewing.scroll.offset, 5);
    assert!(!reviewing.scroll.follow_tail);
}

/// Verifies unknown lifecycle deltas and finishes are ignored deterministically.
#[test]
fn unknown_lifecycle_ids_are_ignored() {
    let mut state = state();
    state.session.prompt = PromptState::from_text("kept");

    reduce(
        &mut state,
        ChatTuiAction::MessageDelta {
            id: item_id("missing-assistant"),
            text: "ignored".to_owned(),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::ReasoningDelta {
            id: item_id("missing-reasoning"),
            text: "ignored".to_owned(),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::ToolCallDelta {
            tool_call_id: "missing-tool".to_owned(),
            text: "ignored".to_owned(),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::CommandOutput {
            command_id: "missing-command".to_owned(),
            text: "ignored".to_owned(),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::CommandFinished {
            command_id: "missing-command".to_owned(),
            exit_code: Some(1),
        },
    );

    assert!(state.session.transcript.is_empty());
    assert_eq!(state.session.prompt, PromptState::from_text("kept"));
    assert_eq!(state.status, Status::Idle);
}
