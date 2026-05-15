use spectacular_tui::{
    app_render_lines, reduce, ChatTuiAction, CommandDisplayChunk, CommandDisplayStatus,
    DisplayLine, DisplayLineStyle, DisplayMetadata, ReasoningLevel, RuntimeSelection, Session,
    SessionId, State, ToolDisplayStatus, TranscriptItemContent, TranscriptItemId,
};

/// Builds representative runtime metadata for tool and command parity tests.
fn runtime() -> RuntimeSelection {
    RuntimeSelection::new(
        "openai-compatible",
        "provider",
        "model",
        ReasoningLevel::Low,
        Some(4096),
    )
}

/// Builds display metadata for tool and command parity tests.
fn display() -> DisplayMetadata {
    DisplayMetadata::new("provider", "model", "low", "/workspace", "session", None)
}

/// Builds initialized TUI state for tool and command parity tests.
fn state() -> State {
    State::new(SessionId::new("session-1"), runtime(), display())
}

/// Builds a stable transcript item id.
fn id(value: &str) -> TranscriptItemId {
    TranscriptItemId::new(value)
}

/// Builds one display line for action payloads.
fn line(text: &str, style: DisplayLineStyle) -> DisplayLine {
    DisplayLine::new(text, style)
}

/// Returns the current render model as plain text and semantic row style pairs.
fn rendered_lines(state: &State) -> Vec<(String, DisplayLineStyle)> {
    app_render_lines(state)
        .into_iter()
        .filter_map(|line| {
            let span = line.spans.first()?;
            Some((line.plain_text(), DisplayLineStyle::from(span.style)))
        })
        .collect()
}

/// Verifies display-ready tool calls preserve adapter-provided call and argument lines.
#[test]
fn tool_display_payload_renders_call_and_arguments() {
    let mut state = state();

    reduce(
        &mut state,
        ChatTuiAction::ToolDisplayStarted {
            id: id("tool-1"),
            tool_call_id: "call-1".to_owned(),
            name: "read".to_owned(),
            call_line: line("Read README.md", DisplayLineStyle::Tool),
            argument_lines: vec![line("path: README.md", DisplayLineStyle::Dim)],
        },
    );

    let rendered = rendered_lines(&state);
    assert!(rendered.contains(&("Read README.md".to_owned(), DisplayLineStyle::Tool)));
    assert!(rendered.contains(&("path: README.md".to_owned(), DisplayLineStyle::Dim)));
}

/// Verifies repeated display-ready tool starts update one existing transcript item.
#[test]
fn tool_display_updates_same_transcript_item() {
    let mut state = state();
    let first_id = id("tool-1");

    reduce(
        &mut state,
        ChatTuiAction::ToolDisplayStarted {
            id: first_id.clone(),
            tool_call_id: "call-1".to_owned(),
            name: "read".to_owned(),
            call_line: line("Read README.md", DisplayLineStyle::Tool),
            argument_lines: vec![line("path: README.md", DisplayLineStyle::Dim)],
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::ToolDisplayStarted {
            id: id("tool-ignored"),
            tool_call_id: "call-1".to_owned(),
            name: "read".to_owned(),
            call_line: line("Read src/lib.rs", DisplayLineStyle::Tool),
            argument_lines: vec![line("path: src/lib.rs", DisplayLineStyle::Dim)],
        },
    );

    assert_eq!(state.session.transcript.len(), 1);
    assert_eq!(state.session.transcript[0].id, first_id);
    let rendered = rendered_lines(&state);
    assert!(rendered.contains(&("Read src/lib.rs".to_owned(), DisplayLineStyle::Tool)));
    assert!(rendered.contains(&("path: src/lib.rs".to_owned(), DisplayLineStyle::Dim)));
    assert!(!rendered.contains(&("Read README.md".to_owned(), DisplayLineStyle::Tool)));
}

/// Verifies line-level display styles survive reducer storage and rendering.
#[test]
fn tool_display_payload_preserves_line_styles() {
    let mut state = state();

    reduce(
        &mut state,
        ChatTuiAction::ToolDisplayStarted {
            id: id("tool-1"),
            tool_call_id: "call-1".to_owned(),
            name: "grep".to_owned(),
            call_line: line("grep pattern", DisplayLineStyle::Tool),
            argument_lines: vec![],
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::ToolDisplayFinished {
            tool_call_id: "call-1".to_owned(),
            status: ToolDisplayStatus::Succeeded,
            output_lines: vec![line("match", DisplayLineStyle::CommandOutput)],
        },
    );

    assert!(matches!(
        &state.session.transcript[0].content,
        TranscriptItemContent::ToolCall(item)
            if item.display.as_ref().unwrap().output_lines[0].style == DisplayLineStyle::CommandOutput
    ));
    assert!(
        rendered_lines(&state).contains(&("match".to_owned(), DisplayLineStyle::CommandOutput,))
    );
}

/// Verifies failed tool display payloads use error styling without TUI-side inference.
#[test]
fn tool_failed_status_uses_error_style() {
    let mut state = state();

    reduce(
        &mut state,
        ChatTuiAction::ToolDisplayStarted {
            id: id("tool-1"),
            tool_call_id: "call-1".to_owned(),
            name: "write".to_owned(),
            call_line: line("write file", DisplayLineStyle::Tool),
            argument_lines: vec![],
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::ToolDisplayFinished {
            tool_call_id: "call-1".to_owned(),
            status: ToolDisplayStatus::Failed,
            output_lines: vec![line("Error: denied", DisplayLineStyle::Error)],
        },
    );

    assert!(matches!(
        &state.session.transcript[0].content,
        TranscriptItemContent::ToolCall(item) if item.status == spectacular_tui::ToolStatus::Failed
    ));
    assert!(
        rendered_lines(&state).contains(&("Error: denied".to_owned(), DisplayLineStyle::Error,))
    );
}

/// Verifies adapter-provided diff output styles render as semantic diff rows.
#[test]
fn tool_diff_lines_use_diff_styles() {
    let mut state = state();

    reduce(
        &mut state,
        ChatTuiAction::ToolDisplayStarted {
            id: id("tool-1"),
            tool_call_id: "call-1".to_owned(),
            name: "edit".to_owned(),
            call_line: line("Edited src/lib.rs", DisplayLineStyle::Tool),
            argument_lines: vec![],
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::ToolDisplayFinished {
            tool_call_id: "call-1".to_owned(),
            status: ToolDisplayStatus::Succeeded,
            output_lines: vec![
                line("1 -old", DisplayLineStyle::DiffRemoved),
                line("1 +new", DisplayLineStyle::DiffAdded),
            ],
        },
    );

    let rendered = rendered_lines(&state);
    assert!(rendered.contains(&("1 -old".to_owned(), DisplayLineStyle::DiffRemoved)));
    assert!(rendered.contains(&("1 +new".to_owned(), DisplayLineStyle::DiffAdded)));
}

/// Verifies command display payloads stream output chunks into a single item.
#[test]
fn command_display_payload_streams_output() {
    let mut state = state();

    reduce(
        &mut state,
        ChatTuiAction::CommandDisplayStarted {
            id: id("command-1"),
            command_id: "cmd-1".to_owned(),
            command_line: line("/git status", DisplayLineStyle::Command),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::CommandDisplayOutput {
            command_id: "cmd-1".to_owned(),
            chunk: CommandDisplayChunk::new("• clean", DisplayLineStyle::CommandOutput),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::CommandDisplayOutput {
            command_id: "cmd-1".to_owned(),
            chunk: CommandDisplayChunk::new("• branch main", DisplayLineStyle::CommandOutput),
        },
    );

    assert_eq!(state.session.transcript.len(), 1);
    let rendered = rendered_lines(&state);
    assert!(rendered.contains(&("/git status".to_owned(), DisplayLineStyle::Command)));
    assert!(rendered.contains(&("• clean".to_owned(), DisplayLineStyle::CommandOutput,)));
    assert!(rendered.contains(&("• branch main".to_owned(), DisplayLineStyle::CommandOutput,)));
}

/// Verifies repeated display-ready command starts update one existing transcript item.
#[test]
fn command_display_updates_same_transcript_item() {
    let mut state = state();
    let first_id = id("command-1");

    reduce(
        &mut state,
        ChatTuiAction::CommandDisplayStarted {
            id: first_id.clone(),
            command_id: "cmd-1".to_owned(),
            command_line: line("/git status", DisplayLineStyle::Command),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::CommandDisplayStarted {
            id: id("command-ignored"),
            command_id: "cmd-1".to_owned(),
            command_line: line("/git diff", DisplayLineStyle::Command),
        },
    );

    assert_eq!(state.session.transcript.len(), 1);
    assert_eq!(state.session.transcript[0].id, first_id);
    let rendered = rendered_lines(&state);
    assert!(rendered.contains(&("/git diff".to_owned(), DisplayLineStyle::Command)));
    assert!(!rendered.contains(&("/git status".to_owned(), DisplayLineStyle::Command)));
}

/// Verifies interleaved display-ready command output updates matching transcript items.
#[test]
fn interleaved_command_display_output_updates_correct_items() {
    let mut state = state();

    reduce(
        &mut state,
        ChatTuiAction::CommandDisplayStarted {
            id: id("command-1"),
            command_id: "cmd-1".to_owned(),
            command_line: line("/git status", DisplayLineStyle::Command),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::CommandDisplayStarted {
            id: id("command-2"),
            command_id: "cmd-2".to_owned(),
            command_line: line("/git diff", DisplayLineStyle::Command),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::CommandDisplayOutput {
            command_id: "cmd-2".to_owned(),
            chunk: CommandDisplayChunk::new("• diff", DisplayLineStyle::CommandOutput),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::CommandDisplayOutput {
            command_id: "cmd-1".to_owned(),
            chunk: CommandDisplayChunk::new("• clean", DisplayLineStyle::CommandOutput),
        },
    );

    assert!(matches!(
        &state.session.transcript[0].content,
        TranscriptItemContent::Command(item)
            if item.display.as_ref().unwrap().output_lines[0].text == "• clean"
    ));
    assert!(matches!(
        &state.session.transcript[1].content,
        TranscriptItemContent::Command(item)
            if item.display.as_ref().unwrap().output_lines[0].text == "• diff"
    ));
}

/// Verifies failed command display status renders adapter-provided exit metadata.
#[test]
fn command_failed_status_renders_exit_metadata() {
    let mut state = state();

    reduce(
        &mut state,
        ChatTuiAction::CommandDisplayStarted {
            id: id("command-1"),
            command_id: "cmd-1".to_owned(),
            command_line: line("false", DisplayLineStyle::Command),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::CommandDisplayFinished {
            command_id: "cmd-1".to_owned(),
            status: CommandDisplayStatus::Failed,
            exit_code: Some(7),
            summary_line: Some(line("failed with exit 7", DisplayLineStyle::Error)),
        },
    );

    let rendered = rendered_lines(&state);
    assert!(rendered.contains(&("failed with exit 7".to_owned(), DisplayLineStyle::Error,)));
}

/// Verifies session snapshots persist display-ready tool payloads and styles.
#[test]
fn snapshot_replay_preserves_tool_display_payload() {
    let mut state = state();
    reduce(
        &mut state,
        ChatTuiAction::ToolDisplayStarted {
            id: id("tool-1"),
            tool_call_id: "call-1".to_owned(),
            name: "edit".to_owned(),
            call_line: line("Edited src/lib.rs", DisplayLineStyle::Tool),
            argument_lines: vec![],
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::ToolDisplayFinished {
            tool_call_id: "call-1".to_owned(),
            status: ToolDisplayStatus::Succeeded,
            output_lines: vec![line("1 +new", DisplayLineStyle::DiffAdded)],
        },
    );

    let value = serde_json::to_value(&state.session).unwrap();
    let restored: Session = serde_json::from_value(value).unwrap();
    let restored_state = State::from_session(restored, Vec::new(), runtime(), display());

    assert!(rendered_lines(&restored_state)
        .contains(&("1 +new".to_owned(), DisplayLineStyle::DiffAdded,)));
}

/// Verifies session snapshots persist display-ready command payloads and styles.
#[test]
fn snapshot_replay_preserves_command_display_payload() {
    let mut state = state();
    reduce(
        &mut state,
        ChatTuiAction::CommandDisplayStarted {
            id: id("command-1"),
            command_id: "cmd-1".to_owned(),
            command_line: line("cargo test", DisplayLineStyle::Command),
        },
    );
    reduce(
        &mut state,
        ChatTuiAction::CommandDisplayOutput {
            command_id: "cmd-1".to_owned(),
            chunk: CommandDisplayChunk::new("• ok", DisplayLineStyle::CommandOutput),
        },
    );

    let value = serde_json::to_value(&state.session).unwrap();
    let restored: Session = serde_json::from_value(value).unwrap();
    let restored_state = State::from_session(restored, Vec::new(), runtime(), display());

    assert!(rendered_lines(&restored_state)
        .contains(&("• ok".to_owned(), DisplayLineStyle::CommandOutput,)));
}

/// Verifies older snapshots without display payloads still render safely.
#[test]
fn missing_display_payload_falls_back_without_panic() {
    let session: Session = serde_json::from_value(serde_json::json!({
        "id": "session-1",
        "prompt": { "text": "", "cursor": 0, "mode": "Regular" },
        "transcript": [
            {
                "id": "tool-1",
                "timestamp": 0,
                "content": {
                    "kind": "ToolCall",
                    "data": {
                        "tool_call_id": "call-1",
                        "name": "grep",
                        "arguments_preview": "pattern",
                        "status": "Finished",
                        "output_preview": "match"
                    }
                }
            },
            {
                "id": "command-1",
                "timestamp": 0,
                "content": {
                    "kind": "Command",
                    "data": {
                        "command_id": "cmd-1",
                        "command": "cargo test",
                        "status": "Failed",
                        "output": "failed\n",
                        "exit_code": 7
                    }
                }
            }
        ]
    }))
    .unwrap();
    let restored_state = State::from_session(session, Vec::new(), runtime(), display());

    let rendered = rendered_lines(&restored_state);
    assert!(rendered.contains(&("grep pattern".to_owned(), DisplayLineStyle::Tool)));
    assert!(rendered.contains(&("match".to_owned(), DisplayLineStyle::CommandOutput)));
    assert!(rendered.contains(&("$ cargo test".to_owned(), DisplayLineStyle::Command)));
    assert!(rendered.contains(&("failed".to_owned(), DisplayLineStyle::CommandOutput)));
    assert!(rendered.contains(&("exit: 7".to_owned(), DisplayLineStyle::Error)));
}
