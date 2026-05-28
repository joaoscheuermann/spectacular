use super::*;

#[test]
fn render_state_to_string_when_tool_display_payload_renders_call_and_arguments() {
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
fn render_state_to_string_when_tool_display_updates_same_transcript_item() {
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
fn render_state_to_string_when_tool_display_payload_preserves_line_styles() {
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
fn render_state_to_string_when_tool_failed_status_uses_error_style() {
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
        TranscriptItemContent::ToolCall(item) if item.status == tui::ToolStatus::Failed
    ));
    assert!(
        rendered_lines(&state).contains(&("Error: denied".to_owned(), DisplayLineStyle::Error,))
    );
}

/// Verifies adapter-provided diff output styles render as semantic diff rows.
#[test]
fn render_state_to_string_when_tool_diff_lines_use_diff_styles() {
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
