use super::*;

#[test]
fn render_app_when_prompt_rows_use_original_marker_and_continuation_indentation() {
    let mut state = state();
    state.session.prompt = tui::PromptState::from_text("first\nsecond");

    let text = visible_text(&app_render_lines(&state));

    assert!(text.contains(&"> first".to_string()));
    assert!(text.contains(&"  second ".to_string()));
    assert!(!text.iter().any(|line| line.contains('█')));
}

#[test]
fn render_app_when_historical_user_prompt_renders_without_prompt_marker() {
    let mut state = state();
    state.session.transcript.push(item(
        1,
        TranscriptItemContent::UserPrompt(tui::UserPromptItem::new("hello\nthere")),
    ));

    let text = visible_text(&app_render_lines(&state));

    assert!(text.contains(&"hello".to_string()));
    assert!(text.contains(&"there".to_string()));
}

#[test]
fn render_app_when_semantic_event_rows_use_original_casing_and_prefixes() {
    let mut state = state();
    state.session.transcript = vec![
        item(
            1,
            TranscriptItemContent::AssistantMessage(tui::AssistantMessageItem::new("hi there")),
        ),
        item(
            2,
            TranscriptItemContent::Warning(tui::WarningItem::new("careful")),
        ),
        item(
            3,
            TranscriptItemContent::Error(tui::ErrorItem::new("boom", Some("details".to_string()))),
        ),
        item(
            4,
            TranscriptItemContent::Success(tui::SuccessItem::new("done")),
        ),
        item(
            5,
            TranscriptItemContent::Notice(tui::NoticeItem::new("plain notice")),
        ),
        item(
            6,
            TranscriptItemContent::Cancellation(tui::CancellationItem::new("user stopped run")),
        ),
        item(
            7,
            TranscriptItemContent::ToolCall(tui::ToolCallItem {
                tool_call_id: "tool-1".to_string(),
                name: "grep".to_string(),
                arguments_preview: Some("pattern".to_string()),
                status: ToolStatus::Finished,
                output_preview: Some("match".to_string()),
                display: None,
            }),
        ),
        item(
            8,
            TranscriptItemContent::Command(tui::CommandItem {
                command_id: "cmd-1".to_string(),
                command: "cargo test".to_string(),
                status: CommandStatus::Failed,
                output: "failure output".to_string(),
                exit_code: Some(101),
                display: None,
            }),
        ),
    ];

    let output = render(&state);

    assert!(output.contains("warning: careful"));
    assert!(output.contains("error: boom"));
    assert!(!output.contains("Assistant:"));
    assert!(!output.contains("Tool:"));
    assert!(!output.contains("Command:"));
    assert!(!output.contains("Warning:"));
    assert!(!output.contains("Error:"));
    assert!(!output.contains("Success:"));
}

#[test]
fn render_app_when_working_line_matches_original_shape() {
    let mut state = state();
    state.status = Status::Running {
        activity: tui::Activity::WaitingForModel,
        cancellable: true,
    };

    let output = render(&state);

    assert!(output.contains("⠙ Working (Esc to cancel)"));
    assert!(!output.contains('�'));
}

#[test]
fn render_app_when_worked_summary_matches_original_shape() {
    let mut state = state();
    state.session.transcript.push(item(
        1,
        TranscriptItemContent::WorkedSummary(tui::WorkedSummaryItem::new("3s", Some(77))),
    ));

    let output = render(&state);

    assert!(output.contains("Worked for 3s · total 77 tokens"));
    assert!(!output.contains('�'));
}

#[test]
fn render_app_when_active_render_applies_semantic_styles() {
    let mut state = state();
    state.session.transcript = vec![
        item(
            1,
            TranscriptItemContent::UserPrompt(tui::UserPromptItem::new("hello")),
        ),
        item(
            2,
            TranscriptItemContent::AssistantMessage(tui::AssistantMessageItem::new("hi")),
        ),
        item(
            3,
            TranscriptItemContent::Warning(tui::WarningItem::new("careful")),
        ),
    ];

    let styles: Vec<RenderStyle> = app_render_lines(&state)
        .into_iter()
        .flat_map(|line| line.spans.into_iter().map(|span| span.style))
        .collect();

    assert!(styles.contains(&RenderStyle::User));
    assert!(styles.contains(&RenderStyle::Assistant));
    assert!(styles.contains(&RenderStyle::Warning));
    assert!(styles.contains(&RenderStyle::Dim));
}
