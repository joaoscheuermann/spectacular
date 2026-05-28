use super::*;

#[test]
fn render_state_to_string_when_command_display_payload_streams_output() {
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
fn render_state_to_string_when_command_display_updates_same_transcript_item() {
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
fn render_state_to_string_when_interleaved_command_display_output_updates_correct_items() {
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

/// Verifies failed command display status renders adapter-provided legacy-styled summary.
#[test]
fn render_state_to_string_when_command_failed_status_renders_legacy_styled_summary() {
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
            summary_line: Some(line("error: failed with exit 7", DisplayLineStyle::Error)),
        },
    );

    let rendered = rendered_lines(&state);
    assert!(rendered.contains(&(
        "error: failed with exit 7".to_owned(),
        DisplayLineStyle::Error,
    )));
}
