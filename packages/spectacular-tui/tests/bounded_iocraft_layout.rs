use futures::{self, StreamExt};
use iocraft::prelude::*;
use spectacular_tui::{
    app_element, effects, reduce, render_state_to_string, semantic_iocraft_style, ChatTuiAction,
    CommandDisplayChunk, CommandDisplayStatus, DisplayLine, DisplayLineStyle, DisplayMetadata,
    DisplaySpan, EventEffect, PromptState, ReasoningLevel, RenderStyle, RuntimeSelection,
    SessionId, State, TranscriptItemId, TuiRgb, TuiSelectionColors,
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
    let mut state = State::new(SessionId::new("session-123"), runtime(), display());
    state.selection_colors = TuiSelectionColors::default();
    state
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
    let mut app = app_element(state.clone(), Some(width), Some(height));
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
    let width = props.width.unwrap_or(80);
    let height = props.height.unwrap_or(24);
    let mut system = hooks.use_context_mut::<SystemContext>();
    let mut state = hooks.use_state(|| state);
    let mut should_exit = hooks.use_state(|| false);

    hooks.use_terminal_events(move |event| {
        if let TerminalEvent::Key(KeyEvent {
            code: KeyCode::Char('q'),
            kind: KeyEventKind::Press,
            ..
        }) = event
        {
            should_exit.set(true);
            return;
        }

        let mut next = state.read().clone();
        reduce(&mut next, ChatTuiAction::Resize { width, height });
        spectacular_tui::apply_view_action_to_state(
            &mut next,
            spectacular_tui::ViewAction::Resize { width, height },
        );
        for effect in effects(&next, event) {
            match effect {
                EventEffect::Action(action) => reduce(&mut next, *action),
                EventEffect::ViewAction(action) => {
                    spectacular_tui::apply_view_action_to_state(&mut next, *action);
                }
                EventEffect::RequestExit => {}
            }
        }
        state.set(next);
    });

    if should_exit.get() {
        system.exit();
    }

    let state = state.read().clone();
    app_element(state, Some(width), Some(height))
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
        matches!(cell.text(), Some("\u{2503}" | "\u{2502}"))
            && cell.text_style().is_some_and(|style| style.color.is_some())
    })
}

/// Returns true when a rendered cell uses the solid cursor background.
fn has_cursor_background(canvas: &Canvas, x: usize, y: usize) -> bool {
    canvas.cell(x, y).is_some_and(|cell| {
        cell.text_style().is_some_and(|style| {
            style.background_color == Some(TuiSelectionColors::default().cursor.iocraft_color())
                && !style.invert
        })
    })
}

/// Returns true when a rendered cell uses the supplied selection colors and semantic weight.
fn has_selection_colors_with_style(
    canvas: &Canvas,
    x: usize,
    y: usize,
    colors: TuiSelectionColors,
    render_style: RenderStyle,
) -> bool {
    let (_, weight) = semantic_iocraft_style(render_style);
    canvas.cell(x, y).is_some_and(|cell| {
        cell.text_style().is_some_and(|style| {
            style.color == Some(colors.selected_text().iocraft_color())
                && style.background_color == Some(colors.background.iocraft_color())
                && style.weight == weight
                && !style.invert
        })
    })
}

/// Builds non-default selection background colors for canvas rendering tests.
fn custom_selection_background() -> TuiSelectionColors {
    TuiSelectionColors {
        text: None,
        background: TuiRgb::new(171, 205, 239),
        cursor: TuiSelectionColors::default().cursor,
    }
}

/// Returns true when a rendered cell uses the semantic style expected for transcript text.
fn has_render_style(canvas: &Canvas, x: usize, y: usize, render_style: RenderStyle) -> bool {
    let (color, weight) = semantic_iocraft_style(render_style);
    canvas.cell(x, y).is_some_and(|cell| {
        cell.text_style()
            .is_some_and(|style| style.color == color && style.weight == weight)
    })
}

// Verifies prompt line breaks render as separate IOCraft terminal rows.

#[path = "bounded_iocraft_layout/bounded_prompt_selection.rs"]
mod bounded_prompt_selection;
#[path = "bounded_iocraft_layout/bounded_transcript_layout.rs"]
mod bounded_transcript_layout;
