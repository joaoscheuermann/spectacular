use spectacular_tui::components::{
    transcript_item_layout_rows, transcript_item_lines, transcript_layout_item_range,
    transcript_layout_row_starts, transcript_layout_total_rows, wrapped_layout_text_rows,
};
use spectacular_tui::{
    footer_left_render_line, footer_render_line, footer_render_line_with_width,
    footer_right_render_line, footer_text_with_width, reduce, selected_text, ChatTuiAction,
    DisplayLine, DisplayLineStyle, DisplayMetadata, ErrorItem, PromptState, ReasoningLevel,
    RenderHighlight, RenderStyle, RuntimeSelection, SelectableProjection, SelectableSurface,
    SessionId, State, Status, Timestamp, TranscriptItem, TranscriptItemContent, TranscriptItemId,
    UserPromptItem, COPIED_SELECTION_NOTICE,
};
use unicode_width::UnicodeWidthStr;

/// Builds an initialized state with stable metadata for transcript projection tests.
fn state() -> State {
    State::new(
        SessionId::new("session-1"),
        RuntimeSelection::new(
            "openai-compatible",
            "openrouter",
            "gpt-5.1",
            ReasoningLevel::High,
            Some(200_000),
        ),
        DisplayMetadata::new(
            "OpenRouter",
            "GPT 5.1",
            "high",
            "/workspace/spectacular",
            "session-1",
            None,
        ),
    )
}

/// Builds a no-wrap command item with a predictable number of render rows.
fn command_item(index: usize, row_count: usize) -> TranscriptItem {
    let output_lines = (1..row_count)
        .map(|row| {
            DisplayLine::new(
                format!("output {index}.{row}"),
                DisplayLineStyle::CommandOutput,
            )
        })
        .collect();

    TranscriptItem::new(
        TranscriptItemId::new(format!("command-{index}")),
        Timestamp::new(index as u64),
        TranscriptItemContent::Command(spectacular_tui::CommandItem {
            command_id: format!("command-{index}"),
            command: format!("cargo test {index}"),
            status: spectacular_tui::CommandStatus::Finished,
            output: String::new(),
            exit_code: Some(0),
            display: Some(spectacular_tui::CommandDisplay {
                command_line: Some(DisplayLine::new(
                    format!("$ {}", "x".repeat(80)),
                    DisplayLineStyle::Command,
                )),
                output_lines,
                summary_line: None,
            }),
        }),
    )
}

// Verifies submitted user prompts render without the active-prompt marker.

fn user_prompt_item(text: &str) -> TranscriptItem {
    TranscriptItem::new(
        TranscriptItemId::new("prompt-1"),
        Timestamp::new(1),
        TranscriptItemContent::UserPrompt(UserPromptItem::new(text)),
    )
}

fn state_with_copied_selection_feedback(width: u16) -> State {
    let mut state = state();
    state.session.prompt = PromptState::from_text("select me");
    reduce(&mut state, ChatTuiAction::Resize { width, height: 6 });
    state.app_selection =
        spectacular_tui::selection::start_selection_at(&state, 2, 0).expect("prompt point");
    state.app_selection =
        spectacular_tui::selection::drag_selection_to(&state, 8, 0).expect("prompt drag");
    spectacular_tui::apply_view_action_to_state(
        &mut state,
        spectacular_tui::ViewAction::RenderedSelectionFeedbackReported {
            message: COPIED_SELECTION_NOTICE.to_owned(),
        },
    );
    state
}

// Verifies no-wrap transcript items count semantic rows, not terminal columns.

#[path = "projection/projection_footer.rs"]
mod projection_footer;
#[path = "projection/projection_layout.rs"]
mod projection_layout;
#[path = "projection/projection_selection.rs"]
mod projection_selection;
#[path = "projection/projection_transcript.rs"]
mod projection_transcript;
