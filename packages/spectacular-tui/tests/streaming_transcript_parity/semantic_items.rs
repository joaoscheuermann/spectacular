use super::*;

#[test]
fn render_state_to_string_when_reasoning_renders_with_original_style_semantics() {
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
fn render_state_to_string_when_command_output_streams_into_existing_item() {
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
fn render_state_to_string_when_command_failure_renders_exit_state() {
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
fn render_state_to_string_when_tool_lifecycle_updates_existing_item() {
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
fn render_state_to_string_when_warning_error_success_cancellation_shapes_match_original() {
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
