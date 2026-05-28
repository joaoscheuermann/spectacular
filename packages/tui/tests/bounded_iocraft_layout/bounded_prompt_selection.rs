use super::*;

#[test]

fn render_app_canvas_when_multiline_prompt_renders_explicit_rows_on_canvas() {
    let mut state = state();

    state.session.prompt = PromptState::from_text("first\nsecond");

    let canvas = render_canvas(&state, 80, 8);

    let lines = canvas_text_lines(&canvas, 80, 8);

    assert_eq!(lines[0], "> first");

    assert_eq!(lines[1], "  second ");

    assert!(has_cursor_background(&canvas, 8, 1));

    assert_eq!(lines[2], "");

    assert!(lines[3].contains("/workspace/doric"));
}

/// Verifies short transcript content starts at the top and leaves unused rows below chrome.

#[test]

fn render_app_canvas_when_short_transcript_starts_at_top_without_bottom_anchoring() {
    let mut state = state();

    reduce(
        &mut state,
        ChatTuiAction::NoticeReported {
            message: "top transcript row".to_string(),
        },
    );

    let canvas = render_canvas(&state, 80, 10);

    let lines = canvas_text_lines(&canvas, 80, 10);

    assert_eq!(lines[0], "top transcript row");

    assert_eq!(lines[1], "");

    assert_eq!(lines[2], ">  What we are going to build today?");

    assert!(has_cursor_background(&canvas, 2, 2));

    assert_eq!(lines[3], "");

    assert!(lines[4].contains("/workspace/doric"));

    assert!(lines[5..].iter().all(String::is_empty));
}

/// Verifies the prompt cursor uses a solid white background without changing other spans.

#[test]

fn render_app_canvas_when_cursor_uses_solid_white_background_without_changing_other_spans() {
    let state = state();

    let canvas = render_canvas(&state, 80, 8);

    assert!(has_cursor_background(&canvas, 2, 0));

    assert!(has_render_style(&canvas, 0, 0, RenderStyle::User));

    assert!(has_render_style(&canvas, 3, 0, RenderStyle::Dim));
}

/// Verifies prompt selection uses concrete selection colors instead of white text.

#[test]

fn render_app_canvas_when_prompt_selection_uses_concrete_selected_text_color() {
    let mut state = state();

    state.session.prompt = PromptState::from_text("hello");

    state.session.prompt.move_to_start(true);

    let canvas = render_canvas(&state, 80, 8);

    assert!(has_cursor_background(&canvas, 2, 0));

    assert!(has_selection_colors_with_style(
        &canvas,
        3,
        0,
        TuiSelectionColors::default(),
        RenderStyle::Text
    ));
}

/// Verifies rendered prompt selection excludes marker cells while styling real prompt text.

#[test]

fn render_app_canvas_when_rendered_selection_prompt_marker_is_not_styled_but_text_is_selected() {
    let mut state = state();

    state.session.prompt = PromptState::from_text("draft");

    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 80,

            height: 8,
        },
    );

    state.app_selection =
        tui::selection::start_selection_at(&state, 4, 0).expect("prompt text start");

    state.app_selection =
        tui::selection::drag_selection_to(&state, 0, 0).expect("prompt text drag");

    let canvas = render_canvas(&state, 80, 8);

    assert!(!has_selection_colors_with_style(
        &canvas,
        0,
        0,
        TuiSelectionColors::default(),
        RenderStyle::User
    ));

    assert!(!has_selection_colors_with_style(
        &canvas,
        1,
        0,
        TuiSelectionColors::default(),
        RenderStyle::User
    ));

    assert!(has_selection_colors_with_style(
        &canvas,
        2,
        0,
        TuiSelectionColors::default(),
        RenderStyle::Text
    ));

    assert!(has_selection_colors_with_style(
        &canvas,
        3,
        0,
        TuiSelectionColors::default(),
        RenderStyle::Text
    ));
}

/// Verifies virtual transcript cells past real text receive rendered-selection colors.

#[test]

fn render_app_canvas_when_rendered_selection_transcript_virtual_spaces_receive_selection_background(
) {
    let mut state = state();

    reduce(
        &mut state,
        ChatTuiAction::SubmitPrompt {
            id: TranscriptItemId::new("prompt-1"),

            text: "hi".to_owned(),
        },
    );

    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 20,

            height: 6,
        },
    );

    state.app_selection = tui::selection::start_selection_at(&state, 5, 0).expect("virtual start");

    state.app_selection = tui::selection::drag_selection_to(&state, 9, 0).expect("virtual drag");

    let canvas = render_canvas(&state, 20, 6);

    assert!(has_selection_colors_with_style(
        &canvas,
        5,
        0,
        TuiSelectionColors::default(),
        RenderStyle::Text
    ));

    assert!(has_selection_colors_with_style(
        &canvas,
        8,
        0,
        TuiSelectionColors::default(),
        RenderStyle::Text
    ));

    assert!(!has_selection_colors_with_style(
        &canvas,
        2,
        0,
        TuiSelectionColors::default(),
        RenderStyle::User
    ));
}

/// Verifies prompt rendered selection can extend into virtual cells without styling chrome.

#[test]

fn render_app_canvas_when_rendered_selection_prompt_virtual_spaces_skip_marker_and_cursor() {
    let mut state = state();

    state.session.prompt = PromptState::from_text("draft");

    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 20,

            height: 6,
        },
    );

    state.app_selection =
        tui::selection::start_selection_at(&state, 2, 0).expect("prompt text start");

    state.app_selection =
        tui::selection::drag_selection_to(&state, 12, 0).expect("prompt virtual drag");

    let canvas = render_canvas(&state, 20, 6);

    assert!(!has_selection_colors_with_style(
        &canvas,
        0,
        0,
        TuiSelectionColors::default(),
        RenderStyle::User
    ));

    assert!(!has_selection_colors_with_style(
        &canvas,
        1,
        0,
        TuiSelectionColors::default(),
        RenderStyle::User
    ));

    assert!(has_selection_colors_with_style(
        &canvas,
        2,
        0,
        TuiSelectionColors::default(),
        RenderStyle::Text
    ));

    assert!(has_cursor_background(&canvas, 7, 0));

    assert!(!has_selection_colors_with_style(
        &canvas,
        7,
        0,
        TuiSelectionColors::default(),
        RenderStyle::Text
    ));

    assert!(has_selection_colors_with_style(
        &canvas,
        8,
        0,
        TuiSelectionColors::default(),
        RenderStyle::Text
    ));

    assert!(has_selection_colors_with_style(
        &canvas,
        11,
        0,
        TuiSelectionColors::default(),
        RenderStyle::Text
    ));
}

/// Verifies default state selection colors use a concrete selected foreground without inversion.

#[test]

fn render_app_canvas_when_default_selection_background_uses_concrete_selected_text_color() {
    let mut state = state();

    reduce(
        &mut state,
        ChatTuiAction::SubmitPrompt {
            id: TranscriptItemId::new("prompt-1"),

            text: "hello".to_owned(),
        },
    );

    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 80,

            height: 6,
        },
    );

    state.app_selection = tui::selection::start_selection_at(&state, 1, 0).unwrap();

    state.app_selection = tui::selection::drag_selection_to(&state, 4, 0).unwrap();

    let canvas = render_canvas(&state, 80, 6);

    assert!(has_selection_colors_with_style(
        &canvas,
        1,
        0,
        TuiSelectionColors::default(),
        RenderStyle::User
    ));

    assert!(has_selection_colors_with_style(
        &canvas,
        3,
        0,
        TuiSelectionColors::default(),
        RenderStyle::User
    ));

    assert!(has_render_style(&canvas, 0, 0, RenderStyle::User));
}

/// Verifies custom state selection backgrounds use their RGB complement as selected text.

#[test]

fn render_app_canvas_when_custom_selection_background_uses_rgb_complement_text_color() {
    let mut state = state();

    reduce(
        &mut state,
        ChatTuiAction::SubmitPrompt {
            id: TranscriptItemId::new("prompt-1"),

            text: "hello".to_owned(),
        },
    );

    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 80,

            height: 6,
        },
    );

    state.selection_colors = custom_selection_background();

    state.app_selection = tui::selection::start_selection_at(&state, 1, 0).unwrap();

    state.app_selection = tui::selection::drag_selection_to(&state, 4, 0).unwrap();

    let canvas = render_canvas(&state, 80, 6);

    assert_eq!(
        custom_selection_background().selected_text(),
        TuiRgb::new(84, 50, 16)
    );

    assert!(has_selection_colors_with_style(
        &canvas,
        1,
        0,
        custom_selection_background(),
        RenderStyle::User
    ));

    assert!(has_selection_colors_with_style(
        &canvas,
        3,
        0,
        custom_selection_background(),
        RenderStyle::User
    ));

    assert!(has_render_style(&canvas, 0, 0, RenderStyle::User));
}

/// Verifies scrolling past the oldest transcript row does not render blank viewport gaps.

#[tokio::test]

async fn render_app_canvas_when_oldest_row_reaches_viewport_top_clamps_scroll_up() {
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

    let page_up = TerminalEvent::Key(KeyEvent::new(KeyEventKind::Press, KeyCode::PageUp));

    let overscroll_canvas = render_canvas_after_events(
        &state,
        80,
        6,
        vec![
            page_up.clone(),
            page_up.clone(),
            page_up.clone(),
            page_up.clone(),
            page_up,
        ],
    )
    .await;

    let overscroll_lines = canvas_text_lines(&overscroll_canvas, 80, 6);

    assert!(overscroll_lines[0].starts_with("submitted prompt 0"));

    assert!(overscroll_lines[2].starts_with("submitted prompt 1"));

    assert!(overscroll_lines[0..3]
        .iter()
        .enumerate()
        .all(|(index, line)| index % 2 == 1 || !line.is_empty()));

    assert!(has_scrollbar_marker(&overscroll_canvas, 79, 0));

    assert!(has_scrollbar_marker(&overscroll_canvas, 79, 2));
}
