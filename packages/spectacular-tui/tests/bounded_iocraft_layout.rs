use futures::{self, StreamExt};
use iocraft::prelude::*;
use spectacular_tui::{
    components::App, reduce, render_state_to_string, semantic_iocraft_style, ChatTuiAction,
    CommandDisplayChunk, CommandDisplayStatus, DisplayLine, DisplayLineStyle, DisplayMetadata,
    PromptState, ReasoningLevel, RenderStyle, RuntimeSelection, SessionId, State, TranscriptItemId,
};

/// Builds a representative runtime selection for bounded layout tests.
fn runtime() -> RuntimeSelection {
    RuntimeSelection::new(
        "openai-compatible",
        "openrouter",
        "gpt-5.1",
        ReasoningLevel::High,
        Some(200_000),
    )
}

/// Builds display metadata for bounded layout tests.
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

/// Builds an initialized state with stable metadata.
fn state() -> State {
    State::new(SessionId::new("session-123"), runtime(), display())
}

/// Renders state through the IOCraft application path.
fn render_app(state: &State) -> String {
    render_state_to_string(state, Some(100))
}

/// Renders actual IOCraft canvas cells for style assertions.
fn render_canvas(state: &State, width: u16, height: u16) -> Canvas {
    render_app_canvas_with_events(state, width, height, Vec::new())
}

/// Renders the App through IOCraft's terminal loop after applying terminal events.
async fn render_canvas_after_events(
    state: &State,
    width: u16,
    height: u16,
    events: Vec<TerminalEvent>,
) -> Canvas {
    render_app_canvas_after_events(state, width, height, events).await
}

/// Renders the App once as a canvas without terminal events.
fn render_app_canvas_with_events(
    state: &State,
    width: u16,
    height: u16,
    _events: Vec<TerminalEvent>,
) -> Canvas {
    let mut app = element!(App(state: state.clone(), width: Some(width), height: Some(height)));
    app.render(Some(usize::from(width)))
}

/// Renders the App through IOCraft's mock terminal and returns the latest canvas.
async fn render_app_canvas_after_events(
    state: &State,
    width: u16,
    height: u16,
    mut events: Vec<TerminalEvent>,
) -> Canvas {
    events.push(TerminalEvent::Key(KeyEvent::new(
        KeyEventKind::Press,
        KeyCode::Char('q'),
    )));
    let mut app = element!(TestHarness(
        state: state.clone(),
        width: Some(width),
        height: Some(height)
    ));
    let canvases = app
        .mock_terminal_render_loop(MockTerminalConfig::with_events(futures::stream::iter(
            events,
        )))
        .collect::<Vec<_>>()
        .await;

    canvases
        .last()
        .cloned()
        .expect("test harness should render at least one canvas")
}

/// Extracts fixed viewport text rows from an IOCraft canvas.
fn canvas_text_lines(canvas: &Canvas, width: u16, height: u16) -> Vec<String> {
    canvas
        .get_text(0, 0, usize::from(width), usize::from(height))
        .split('\n')
        .map(ToOwned::to_owned)
        .collect()
}

/// Test wrapper that exits after receiving the synthetic quit event.
#[component]
fn TestHarness(mut hooks: Hooks, props: &TestHarnessProps) -> impl Into<AnyElement<'static>> {
    let state = props.state.clone().expect("TestHarness requires state");
    let width = props.width;
    let height = props.height;
    let mut system = hooks.use_context_mut::<SystemContext>();
    let mut should_exit = hooks.use_state(|| false);

    hooks.use_terminal_events(move |event| {
        if let TerminalEvent::Key(KeyEvent {
            code: KeyCode::Char('q'),
            kind: KeyEventKind::Press,
            ..
        }) = event
        {
            should_exit.set(true);
        }
    });

    if should_exit.get() {
        system.exit();
    }

    element!(App(state, width, height))
}

/// Props for the App test harness.
#[derive(Default, Props)]
struct TestHarnessProps {
    state: Option<State>,
    width: Option<u16>,
    height: Option<u16>,
}

/// Counts occurrences of a substring in rendered output.
fn occurrences(output: &str, needle: &str) -> usize {
    output.match_indices(needle).count()
}

/// Returns true when the canvas contains a scrollbar marker at the given cell.
fn has_scrollbar_marker(canvas: &Canvas, x: usize, y: usize) -> bool {
    canvas.cell(x, y).is_some_and(|cell| {
        matches!(cell.text(), Some("│" | "┃"))
            && cell.text_style().is_some_and(|style| style.color.is_some())
    })
}

/// Returns true when a rendered cell uses the explicit selection colors.
fn has_selection_colors(canvas: &Canvas, x: usize, y: usize) -> bool {
    canvas.cell(x, y).is_some_and(|cell| {
        cell.text_style().is_some_and(|style| {
            style.color
                == Some(Color::Rgb {
                    r: 15,
                    g: 23,
                    b: 42,
                })
                && style.background_color
                    == Some(Color::Rgb {
                        r: 240,
                        g: 240,
                        b: 240,
                    })
                && !style.invert
        })
    })
}

/// Returns true when a rendered cell uses the semantic style expected for transcript text.
fn has_render_style(canvas: &Canvas, x: usize, y: usize, render_style: RenderStyle) -> bool {
    let (color, weight) = semantic_iocraft_style(render_style);
    canvas.cell(x, y).is_some_and(|cell| {
        cell.text_style()
            .is_some_and(|style| style.color == color && style.weight == weight)
    })
}

/// Verifies prompt line breaks render as separate IOCraft terminal rows.
#[test]
fn multiline_prompt_renders_explicit_rows_on_canvas() {
    let mut state = state();
    state.session.prompt = PromptState::from_text("first\nsecond");

    let canvas = render_canvas(&state, 80, 8);
    let lines = canvas_text_lines(&canvas, 80, 8);

    assert_eq!(lines[0], "> first");
    assert_eq!(lines[1], "  second ");
    assert!(has_selection_colors(&canvas, 8, 1));
    assert_eq!(lines[2], "");
    assert!(lines[3].contains("/workspace/spectacular"));
}

/// Verifies short transcript content starts at the top and leaves unused rows below chrome.
#[test]
fn short_transcript_starts_at_top_without_bottom_anchoring() {
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
    assert!(has_selection_colors(&canvas, 2, 2));
    assert_eq!(lines[3], "");
    assert!(lines[4].contains("/workspace/spectacular"));
    assert!(lines[5..].iter().all(String::is_empty));
}

/// Verifies scrolling past the oldest transcript row does not render blank viewport gaps.
#[tokio::test]
async fn scroll_up_clamps_when_oldest_row_reaches_viewport_top() {
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
            chunk: CommandDisplayChunk::new("• compiling", DisplayLineStyle::CommandOutput),
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
    assert_eq!(lines[1], "• compiling");
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
    assert_eq!(lines[2].trim_end_matches(['│', '┃']).trim_end(), "");
    assert_eq!(lines[3], ">  What we are going to build today?");
    assert!(has_selection_colors(&canvas, 2, 3));
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
