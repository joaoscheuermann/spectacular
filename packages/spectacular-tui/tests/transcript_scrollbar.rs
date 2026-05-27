use iocraft::prelude::*;
use spectacular_tui::{
    app_element, apply_view_action_to_state, reduce, ChatTuiAction, DisplayMetadata,
    ReasoningLevel, RuntimeSelection, SessionId, State, TranscriptItemId, ViewAction,
};

const THUMB_COLOR: Color = Color::Rgb {
    r: 71,
    g: 85,
    b: 105,
};

const TRACK_COLOR: Color = Color::Rgb {
    r: 30,
    g: 41,
    b: 59,
};

fn runtime() -> RuntimeSelection {
    RuntimeSelection::new(
        "openai-compatible",
        "openrouter",
        "gpt-5.1",
        ReasoningLevel::High,
        Some(200_000),
    )
}

fn display() -> DisplayMetadata {
    DisplayMetadata::new(
        "OpenRouter",
        "GPT 5.1",
        "high",
        "/workspace/spectacular",
        "session-123",
        None,
    )
}

fn state() -> State {
    State::new(SessionId::new("session-123"), runtime(), display())
}

fn submit_prompts(state: &mut State, count: usize) {
    for index in 0..count {
        reduce(
            state,
            ChatTuiAction::SubmitPrompt {
                id: TranscriptItemId::new(format!("prompt-{index}")),
                text: format!("submitted prompt {index} {}", "x".repeat(96)),
            },
        );
    }
}

fn resize(state: &mut State, width: u16, height: u16) {
    reduce(state, ChatTuiAction::Resize { width, height });
    apply_view_action_to_state(state, ViewAction::Resize { width, height });
}

fn render_canvas(state: &State, width: u16, height: u16) -> Canvas {
    let mut app = app_element(state.clone(), Some(width), Some(height));
    app.render(Some(usize::from(width)))
}

fn scrollbar_color(canvas: &Canvas, x: usize, y: usize) -> Option<Color> {
    canvas.cell(x, y).and_then(|cell| {
        matches!(cell.text(), Some("\u{2503}" | "\u{2502}"))
            .then(|| cell.text_style().and_then(|style| style.color))
            .flatten()
    })
}

fn scrollbar_thumb_rows(canvas: &Canvas, x: usize, height: u16) -> Vec<usize> {
    (0..usize::from(height))
        .filter(|row| scrollbar_color(canvas, x, *row) == Some(THUMB_COLOR))
        .collect()
}

fn scrollbar_track_rows(canvas: &Canvas, x: usize, height: u16) -> Vec<usize> {
    (0..usize::from(height))
        .filter(|row| scrollbar_color(canvas, x, *row) == Some(TRACK_COLOR))
        .collect()
}

#[test]
fn render_app_canvas_when_content_fits_viewport_omits_scrollbar_column() {
    let mut state = state();
    submit_prompts(&mut state, 1);

    let canvas = render_canvas(&state, 80, 10);

    assert!(scrollbar_thumb_rows(&canvas, 79, 10).is_empty());
    assert!(scrollbar_track_rows(&canvas, 79, 10).is_empty());
}

#[test]
fn render_app_canvas_when_scrollbar_rendering_across_terminal_widths_reserves_rightmost_column() {
    let mut state = state();
    submit_prompts(&mut state, 8);

    for width in [20, 40, 80] {
        let canvas = render_canvas(&state, width, 6);
        let scrollbar_x = usize::from(width - 1);

        assert!(!scrollbar_thumb_rows(&canvas, scrollbar_x, 6).is_empty());
        assert!(!scrollbar_track_rows(&canvas, scrollbar_x, 6).is_empty());
    }
}

#[test]
fn render_app_canvas_when_scroll_position_changes_moves_thumb_toward_viewport_top() {
    let mut state = state();
    submit_prompts(&mut state, 12);
    resize(&mut state, 80, 6);

    let tail_canvas = render_canvas(&state, 80, 6);
    let tail_thumb_top = scrollbar_thumb_rows(&tail_canvas, 79, 6)
        .into_iter()
        .next()
        .expect("tail scrollbar thumb");

    apply_view_action_to_state(&mut state, ViewAction::ScrollTranscript(1_000));

    let scrolled_canvas = render_canvas(&state, 80, 6);
    let scrolled_thumb_top = scrollbar_thumb_rows(&scrolled_canvas, 79, 6)
        .into_iter()
        .next()
        .expect("scrolled scrollbar thumb");

    assert!(scrolled_thumb_top < tail_thumb_top);
}
