use spectacular_tui::{
    app_render_lines, reduce, ChatTuiAction, CommandStatus, DisplayMetadata, ReasoningLevel,
    RenderStyle, RuntimeSelection, SessionId, State, ToolStatus, TranscriptItemContent,
    TranscriptItemId,
};

/// Builds representative runtime metadata for streaming parity tests.
fn runtime() -> RuntimeSelection {
    RuntimeSelection::new(
        "openai-compatible",
        "provider",
        "model",
        ReasoningLevel::Low,
        Some(4096),
    )
}

/// Builds display metadata for streaming parity tests.
fn display() -> DisplayMetadata {
    DisplayMetadata::new("provider", "model", "low", "/workspace", "session", None)
}

/// Builds initialized TUI state for streaming parity tests.
fn state() -> State {
    State::new(SessionId::new("session-1"), runtime(), display())
}

/// Builds a stable transcript item id.
fn id(value: &str) -> TranscriptItemId {
    TranscriptItemId::new(value)
}

/// Returns the assistant item text currently visible to the renderer.
fn assistant_text(state: &State) -> &str {
    let TranscriptItemContent::AssistantMessage(item) = &state.session.transcript[0].content else {
        panic!("expected assistant item");
    };
    &item.text
}

/// Verifies assistant deltas are visible as soon as they arrive.
#[test]
fn assistant_delta_is_visible_immediately() {
    let mut state = state();
    let item_id = id("assistant-1");

    reduce(
        &mut state,
        ChatTuiAction::MessageStarted {
            id: item_id.clone(),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::MessageDelta {
            id: item_id,
            text: "streamed text".to_owned(),
        },
    );

    assert_eq!(assistant_text(&state), "streamed text");
}

/// Verifies assistant deltas append directly without reveal chunking.
#[test]
fn assistant_deltas_are_not_typewriter_chunked() {
    let mut state = state();
    let item_id = id("assistant-1");
    let text = format!("{}🙂{}", "a".repeat(30), "b".repeat(69));

    reduce(
        &mut state,
        ChatTuiAction::MessageStarted {
            id: item_id.clone(),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::MessageDelta {
            id: item_id,
            text: text.clone(),
        },
    );

    assert_eq!(assistant_text(&state), text);
}

/// Verifies visible text remains available after stream completion.
#[test]
fn assistant_text_remains_after_stream_finish() {
    let mut state = state();
    let item_id = id("assistant-1");

    reduce(
        &mut state,
        ChatTuiAction::MessageStarted {
            id: item_id.clone(),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::MessageDelta {
            id: item_id.clone(),
            text: "x".repeat(35),
        },
    );
    reduce(&mut state, ChatTuiAction::MessageFinished { id: item_id });

    assert_eq!(assistant_text(&state), "x".repeat(35));
}

/// Verifies assistant deltas do not advance the spinner frame.
#[test]
fn assistant_delta_does_not_advance_spinner() {
    let mut state = state();
    let item_id = id("assistant-1");
    let frame = state.spinner.current_frame().to_owned();

    reduce(
        &mut state,
        ChatTuiAction::MessageStarted {
            id: item_id.clone(),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::MessageDelta {
            id: item_id,
            text: "hello".to_owned(),
        },
    );

    assert_eq!(state.spinner.current_frame(), frame);
}

/// Verifies spinner ticks do not mutate visible assistant text.
#[test]
fn spinner_tick_does_not_mutate_assistant_text() {
    let mut state = state();
    let item_id = id("assistant-1");

    reduce(
        &mut state,
        ChatTuiAction::MessageStarted {
            id: item_id.clone(),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::MessageDelta {
            id: item_id,
            text: "visible".to_owned(),
        },
    );
    reduce(&mut state, ChatTuiAction::SpinnerTick);

    assert_eq!(assistant_text(&state), "visible");
}

/// Verifies reasoning rendering keeps original-style semantic treatment without labels.
#[test]
fn reasoning_renders_with_original_style_semantics() {
    let mut state = state();
    reduce(
        &mut state,
        ChatTuiAction::ReasoningStarted {
            id: id("reasoning-1"),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::ReasoningDelta {
            id: id("reasoning-1"),
            text: "\n thinking\n\n".to_owned(),
        },
    );

    let lines = app_render_lines(&state);
    assert!(lines.iter().any(|line| line.plain_text() == " thinking"));
    assert!(lines.iter().any(|line| line
        .spans
        .iter()
        .any(|span| span.style == RenderStyle::Reasoning)));
    assert!(lines
        .iter()
        .all(|line| !line.plain_text().contains("Reasoning:")));
}

/// Verifies command output streams into an existing command item.
#[test]
fn command_output_streams_into_existing_item() {
    let mut state = state();
    reduce(
        &mut state,
        ChatTuiAction::CommandStarted {
            id: id("command-1"),
            command_id: "cmd-1".to_owned(),
            command: "cargo test".to_owned(),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::CommandOutput {
            command_id: "cmd-1".to_owned(),
            text: "line 1\n".to_owned(),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::CommandOutput {
            command_id: "cmd-1".to_owned(),
            text: "line 2".to_owned(),
        },
    );

    assert_eq!(state.session.transcript.len(), 1);
    assert!(matches!(
        &state.session.transcript[0].content,
        TranscriptItemContent::Command(item) if item.output == "line 1\nline 2"
    ));
}

/// Verifies failed commands render an exit state.
#[test]
fn command_failure_renders_exit_state() {
    let mut state = state();
    reduce(
        &mut state,
        ChatTuiAction::CommandStarted {
            id: id("command-1"),
            command_id: "cmd-1".to_owned(),
            command: "false".to_owned(),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::CommandFinished {
            command_id: "cmd-1".to_owned(),
            exit_code: Some(7),
        },
    );

    let output: Vec<String> = app_render_lines(&state)
        .into_iter()
        .map(|line| line.plain_text())
        .collect();
    assert!(matches!(
        &state.session.transcript[0].content,
        TranscriptItemContent::Command(item) if item.status == CommandStatus::Failed
    ));
    assert!(output.iter().any(|line| line == "exit: 7"));
}

/// Verifies tool lifecycle updates one existing semantic item.
#[test]
fn tool_lifecycle_updates_existing_item() {
    let mut state = state();
    reduce(
        &mut state,
        ChatTuiAction::ToolCallStarted {
            id: id("tool-1"),
            tool_call_id: "call-1".to_owned(),
            name: "grep".to_owned(),
            arguments: "pattern".to_owned(),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::ToolCallDelta {
            tool_call_id: "call-1".to_owned(),
            text: "match".to_owned(),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::ToolCallFailed {
            tool_call_id: "call-1".to_owned(),
            error: "denied".to_owned(),
        },
    );

    assert_eq!(state.session.transcript.len(), 1);
    assert!(matches!(
        &state.session.transcript[0].content,
        TranscriptItemContent::ToolCall(item)
            if item.status == ToolStatus::Failed && item.output_preview.as_deref() == Some("matchdenied")
    ));
}

/// Verifies warning, error, success, and cancellation render in original-shaped lowercase forms.
#[test]
fn warning_error_success_cancellation_shapes_match_original() {
    let mut state = state();
    reduce(
        &mut state,
        ChatTuiAction::WarningReported {
            message: "careful".to_owned(),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::ErrorReported {
            message: "bad".to_owned(),
            details: None,
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::SuccessReported {
            message: "done".to_owned(),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::AgentCancelled {
            reason: "stopped".to_owned(),
        },
    );

    let output: Vec<String> = app_render_lines(&state)
        .into_iter()
        .map(|line| line.plain_text())
        .collect();
    assert!(output.iter().any(|line| line == "warning: careful"));
    assert!(output.iter().any(|line| line == "error: bad"));
    assert!(output.iter().any(|line| line == "done"));
    assert!(output.iter().any(|line| line == "stopped"));
}

/// Verifies following tail keeps newly streamed assistant text visible.
#[test]
fn scroll_follow_mode_tracks_bottom_on_new_output() {
    let mut state = state();
    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 100,
            height: 3,
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::MessageStarted {
            id: id("assistant-1"),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::MessageDelta {
            id: id("assistant-1"),
            text: "tail".to_owned(),
        },
    );

    assert_eq!(state.scroll.offset, 0);
    assert!(state.scroll.follow_tail);
    assert!(app_render_lines(&state)
        .iter()
        .any(|line| line.plain_text() == "tail"));
}

/// Verifies manual review mode is preserved while new output arrives.
#[test]
fn scroll_manual_mode_does_not_snap_on_new_output() {
    let mut state = state();
    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 100,
            height: 2,
        },
    );
    reduce(&mut state, ChatTuiAction::ScrollTranscript(1));
    reduce(
        &mut state,
        ChatTuiAction::MessageStarted {
            id: id("assistant-1"),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::MessageDelta {
            id: id("assistant-1"),
            text: "tail".to_owned(),
        },
    );

    assert_eq!(state.scroll.offset, 2);
    assert!(!state.scroll.follow_tail);
}
