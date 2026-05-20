use spectacular_tui::{
    DisplayLine, DisplayLineStyle, DisplayMetadata, ReasoningLevel, RuntimeSelection, SessionId,
    State, Timestamp, TranscriptItem, TranscriptItemContent, TranscriptItemId,
};

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

/// Verifies no-wrap transcript items count semantic rows, not terminal columns.
#[test]
fn no_wrap_item_row_count_ignores_content_width() {
    let item = command_item(1, 3);

    assert_eq!(spectacular_tui::transcript_item_layout_rows(&item, 4), 4);
    assert_eq!(spectacular_tui::transcript_item_layout_rows(&item, 80), 4);
}

/// Verifies local wrapped row estimation handles core IOCraft text wrapping cases.
#[test]
fn wrapped_text_row_count_matches_core_wrapping_cases() {
    assert_eq!(spectacular_tui::wrapped_layout_text_rows("", 8), 1);
    assert_eq!(
        spectacular_tui::wrapped_layout_text_rows("alpha beta", 20),
        1
    );
    assert_eq!(
        spectacular_tui::wrapped_layout_text_rows("alpha beta", 8),
        2
    );
    assert_eq!(spectacular_tui::wrapped_layout_text_rows("alpha ", 5), 1);
    assert_eq!(spectacular_tui::wrapped_layout_text_rows("abcdefgh", 3), 3);
}

/// Verifies transcript layout row counts are cumulative across semantic items.
#[test]
fn transcript_layout_rows_are_cumulative() {
    let mut state = state();
    state.session.transcript = vec![command_item(0, 2), command_item(1, 3), command_item(2, 1)];

    assert_eq!(
        spectacular_tui::transcript_layout_row_starts(&state, 4),
        vec![0, 3, 7]
    );
    assert_eq!(spectacular_tui::transcript_layout_total_rows(&state, 4), 9);
}

/// Verifies row-window lookup includes every item intersecting the half-open row range.
#[test]
fn transcript_layout_finds_intersecting_item_ranges() {
    let mut state = state();
    state.session.transcript = vec![command_item(0, 2), command_item(1, 3), command_item(2, 1)];

    assert_eq!(
        spectacular_tui::transcript_layout_item_range(&state, 4, 0..1),
        0..1
    );
    assert_eq!(
        spectacular_tui::transcript_layout_item_range(&state, 4, 1..4),
        0..2
    );
    assert_eq!(
        spectacular_tui::transcript_layout_item_range(&state, 4, 2..5),
        0..2
    );
    assert_eq!(
        spectacular_tui::transcript_layout_item_range(&state, 4, 3..6),
        1..2
    );
    assert_eq!(
        spectacular_tui::transcript_layout_item_range(&state, 4, 7..9),
        2..3
    );
    assert_eq!(
        spectacular_tui::transcript_layout_item_range(&state, 4, 4..4),
        0..0
    );
}
