use super::*;

/// Verifies overflowing transcript content renders a scrollbar next to the transcript pane.
#[test]

fn overflowing_transcript_shows_scrollbar() {
    let mut state = state();

    for index in 0..8 {
        reduce(
            &mut state,
            ChatTuiAction::SubmitPrompt {
                id: TranscriptItemId::new(format!("prompt-{index}")),

                text: format!("submitted prompt {index}"),
            },
        );
    }

    let canvas = render_app_canvas_with_events(&state, 80, 6, Vec::new());

    assert!(has_scrollbar_marker(&canvas, 79, 0));

    assert!(has_scrollbar_marker(&canvas, 79, 2));
}

/// Verifies tool-call rows render mixed IOCraft styles matching legacy terminal helpers.

#[test]

fn tool_transcript_call_line_renders_legacy_segment_styles_on_canvas() {
    let mut state = state();

    reduce(
        &mut state,
        ChatTuiAction::ToolDisplayStarted {
            id: TranscriptItemId::new("tool-1"),

            tool_call_id: "call-1".to_string(),

            name: "write".to_string(),

            call_line: DisplayLine::from_spans(vec![
                DisplaySpan::new("Write", DisplayLineStyle::Tool),
                DisplaySpan::new(" ", DisplayLineStyle::Text),
                DisplaySpan::new("README.md", DisplayLineStyle::Text),
                DisplaySpan::new(" ", DisplayLineStyle::Dim),
                DisplaySpan::new("(10 bytes)", DisplayLineStyle::Dim),
            ]),

            argument_lines: Vec::new(),
        },
    );

    let canvas = render_canvas(&state, 80, 8);

    let lines = canvas_text_lines(&canvas, 80, 8);

    assert_eq!(lines[0], "Write README.md (10 bytes)");

    assert!(has_render_style(&canvas, 0, 0, RenderStyle::Tool));

    assert!(has_render_style(&canvas, 6, 0, RenderStyle::Text));

    assert!(has_render_style(&canvas, 16, 0, RenderStyle::Dim));
}

/// Verifies command transcript rows render through IOCraft with legacy text and semantic styles.

#[test]

fn command_transcript_renders_legacy_shape_and_styles_on_canvas() {
    let mut state = state();

    reduce(
        &mut state,
        ChatTuiAction::CommandDisplayStarted {
            id: TranscriptItemId::new("command-1"),

            command_id: "command-1".to_string(),

            command_line: DisplayLine::new("cargo test", DisplayLineStyle::Command),
        },
    );

    reduce(
        &mut state,
        ChatTuiAction::CommandDisplayOutput {
            command_id: "command-1".to_string(),

            chunk: CommandDisplayChunk::new("\u{2022} compiling", DisplayLineStyle::CommandOutput),
        },
    );

    reduce(
        &mut state,
        ChatTuiAction::CommandDisplayFinished {
            command_id: "command-1".to_string(),

            status: CommandDisplayStatus::Failed,

            exit_code: Some(7),

            summary_line: Some(DisplayLine::new(
                "error: failed with exit 7",
                DisplayLineStyle::Error,
            )),
        },
    );

    let canvas = render_canvas(&state, 80, 8);

    let lines = canvas_text_lines(&canvas, 80, 8);

    assert_eq!(lines[0], "cargo test");

    assert_eq!(lines[1], "\u{2022} compiling");

    assert_eq!(lines[2], "error: failed with exit 7");

    assert!(has_render_style(&canvas, 0, 0, RenderStyle::Command));

    assert!(has_render_style(&canvas, 0, 1, RenderStyle::CommandOutput));

    assert!(has_render_style(&canvas, 0, 2, RenderStyle::Error));
}

/// Verifies tail-follow rendering does not overshift when no-wrap rows exceed viewport width.

#[test]

fn no_wrap_transcript_rows_do_not_create_bottom_gap_at_tail() {
    let mut state = state();

    reduce(
        &mut state,
        ChatTuiAction::CommandDisplayStarted {
            id: TranscriptItemId::new("command-1"),

            command_id: "command-1".to_string(),

            command_line: DisplayLine::new(
                format!("$ {}", "x".repeat(120)),
                DisplayLineStyle::Command,
            ),
        },
    );

    for index in 0..4 {
        reduce(
            &mut state,
            ChatTuiAction::CommandDisplayOutput {
                command_id: "command-1".to_string(),

                chunk: CommandDisplayChunk::new(
                    format!("output {index} {}", "y".repeat(120)),
                    DisplayLineStyle::CommandOutput,
                ),
            },
        );
    }

    reduce(
        &mut state,
        ChatTuiAction::CommandDisplayFinished {
            command_id: "command-1".to_string(),

            status: CommandDisplayStatus::Succeeded,

            exit_code: Some(0),

            summary_line: None,
        },
    );

    let canvas = render_canvas(&state, 40, 6);

    let lines = canvas_text_lines(&canvas, 40, 6);

    assert!(lines[0].starts_with("output 2"));

    assert!(lines[1].starts_with("output 3"));

    assert_eq!(
        lines[2]
            .trim_end_matches(['\u{2503}', '\u{2502}'])
            .trim_end(),
        ""
    );

    assert_eq!(lines[3], ">  What we are going to build today?");

    assert!(has_cursor_background(&canvas, 2, 3));

    assert_eq!(lines[4], "");

    assert!(lines[5].contains("/workspace/spectacular"));
}

/// Verifies full-width transcript rows leave the rightmost column for the scrollbar.

#[test]

fn full_width_transcript_rows_do_not_push_scrollbar_out_of_view() {
    let mut state = state();

    for index in 0..8 {
        reduce(
            &mut state,
            ChatTuiAction::SubmitPrompt {
                id: TranscriptItemId::new(format!("prompt-{index}")),

                text: format!("{index}{}", "x".repeat(120)),
            },
        );
    }

    for width in [20, 40, 80] {
        let canvas = render_app_canvas_with_events(&state, width, 6, Vec::new());

        let scrollbar_x = usize::from(width.saturating_sub(1));

        assert!(has_scrollbar_marker(&canvas, scrollbar_x, 0));

        assert!(has_scrollbar_marker(&canvas, scrollbar_x, 2));
    }
}

/// Verifies transcript overflow is bounded without duplicating fixed rows.

#[test]

fn transcript_overflow_is_bounded_above_working_prompt_and_footer() {
    let mut state = state();

    for index in 0..20 {
        reduce(
            &mut state,
            ChatTuiAction::SubmitPrompt {
                id: TranscriptItemId::new(format!("prompt-{index}")),

                text: format!("submitted prompt {index}"),
            },
        );
    }

    reduce(&mut state, ChatTuiAction::AgentStarted);

    state.session.prompt = PromptState::from_text("draft prompt");

    let output = render_app(&state);

    assert!(output.contains("Working (Esc to cancel)"));

    assert_eq!(occurrences(&output, "Working (Esc to cancel)"), 1);

    assert_eq!(occurrences(&output, "> draft prompt"), 1);

    assert!(output.contains("/workspace/spectacular"));

    assert!(output.contains("GPT 5.1 (high)"));

    assert!(output.contains("submitted prompt 0"));

    assert!(output.contains("submitted prompt 19"));
}

/// Verifies streaming updates stay in the bounded transcript region without duplicating footer rows.

#[test]

fn streaming_assistant_updates_remain_bounded_with_fixed_chrome() {
    let mut state = state();

    reduce(
        &mut state,
        ChatTuiAction::SubmitPrompt {
            id: TranscriptItemId::new("prompt-1"),

            text: "hello".to_string(),
        },
    );

    reduce(&mut state, ChatTuiAction::AgentStarted);

    let assistant_id = TranscriptItemId::new("assistant-1");

    reduce(
        &mut state,
        ChatTuiAction::MessageStarted {
            id: assistant_id.clone(),
        },
    );

    reduce(
        &mut state,
        ChatTuiAction::MessageDelta {
            id: assistant_id,

            text: "streaming assistant response".to_string(),
        },
    );

    state.session.prompt = PromptState::from_text("draft prompt");

    let output = render_app(&state);

    assert_eq!(occurrences(&output, "Working (Esc to cancel)"), 1);

    assert_eq!(occurrences(&output, "> draft prompt"), 1);

    assert_eq!(occurrences(&output, "/workspace/spectacular"), 1);

    assert!(output.contains("streaming assistant response"));
}

/// Verifies transcript highlighting is anchored to the selected row, not screen coordinates.
#[test]
fn rendered_selection_transcript_highlight_disappears_and_returns_across_scroll() {
    let mut state = state();

    for index in 0..8 {
        reduce(
            &mut state,
            ChatTuiAction::SubmitPrompt {
                id: TranscriptItemId::new(format!("prompt-{index}")),
                text: format!("submitted prompt {index}"),
            },
        );
    }
    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 80,
            height: 6,
        },
    );
    spectacular_tui::apply_view_action_to_state(
        &mut state,
        spectacular_tui::ViewAction::Resize {
            width: 80,
            height: 6,
        },
    );

    let selected_screen_row = spectacular_tui::SelectableProjection::for_state(&state)
        .rows()
        .iter()
        .find(|row| row.text == "submitted prompt 7")
        .expect("selected prompt row")
        .screen_row;

    state.app_selection =
        spectacular_tui::selection::start_selection_at(&state, 0, selected_screen_row as u16)
            .unwrap();
    state.app_selection =
        spectacular_tui::selection::drag_selection_to(&state, 9, selected_screen_row as u16)
            .unwrap();

    let selected_canvas = render_canvas(&state, 80, 6);
    assert!(has_selection_colors_with_style(
        &selected_canvas,
        0,
        selected_screen_row,
        TuiSelectionColors::default(),
        RenderStyle::User
    ));

    spectacular_tui::apply_view_action_to_state(
        &mut state,
        spectacular_tui::ViewAction::ScrollTranscript(4),
    );
    let scrolled_canvas = render_canvas(&state, 80, 6);
    let scrolled_lines = canvas_text_lines(&scrolled_canvas, 80, 6);

    assert!(scrolled_lines[selected_screen_row].starts_with("submitted prompt 5"));
    assert!(!has_selection_colors_with_style(
        &scrolled_canvas,
        0,
        selected_screen_row,
        TuiSelectionColors::default(),
        RenderStyle::User
    ));

    spectacular_tui::apply_view_action_to_state(
        &mut state,
        spectacular_tui::ViewAction::ScrollTranscript(-4),
    );
    let restored_canvas = render_canvas(&state, 80, 6);
    let restored_lines = canvas_text_lines(&restored_canvas, 80, 6);

    assert!(restored_lines[selected_screen_row].starts_with("submitted prompt 7"));
    assert!(has_selection_colors_with_style(
        &restored_canvas,
        0,
        selected_screen_row,
        TuiSelectionColors::default(),
        RenderStyle::User
    ));
}
