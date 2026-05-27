use super::*;

#[test]
fn render_state_to_string_when_snapshot_replay_preserves_tool_display_payload() {
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
fn render_state_to_string_when_snapshot_replay_preserves_command_display_payload() {
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
fn render_state_to_string_when_missing_display_payload_falls_back_without_panic() {
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
