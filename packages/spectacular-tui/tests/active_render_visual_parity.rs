use spectacular_tui::{
    app_render_lines, footer_render_line, footer_right_render_line, render_state_to_string,
    CommandStatus, ContextTokenUsage, DisplayMetadata, OpeningBannerItem, ReasoningLevel,
    RenderLine, RenderStyle, RuntimeSelection, SessionId, State, Status, ToolStatus,
    TranscriptItem, TranscriptItemContent, TranscriptItemId, TurnTokenUsage, WorktreeMetadata,
};

/// Builds a representative runtime selection for active render parity tests.
fn runtime() -> RuntimeSelection {
    RuntimeSelection::new(
        "openai-compatible",
        "openrouter",
        "gpt-5.1",
        ReasoningLevel::High,
        Some(200_000),
    )
}

/// Builds display metadata for active render parity tests.
fn display(usage: Option<ContextTokenUsage>) -> DisplayMetadata {
    DisplayMetadata::new(
        "OpenRouter",
        "GPT 5.1",
        "high",
        "/workspace/spectacular",
        "session-123",
        usage,
    )
}

/// Builds an initialized state with stable metadata.
fn state() -> State {
    State::new(
        SessionId::new("session-123"),
        runtime_without_context_window(),
        display(None),
    )
}

/// Builds a runtime selection without context usage for legacy no-usage checks.
fn runtime_without_context_window() -> RuntimeSelection {
    RuntimeSelection::new(
        "openai-compatible",
        "openrouter",
        "gpt-5.1",
        ReasoningLevel::High,
        None,
    )
}

/// Creates a transcript item with stable test identity and timestamp.
fn item(index: u64, content: TranscriptItemContent) -> TranscriptItem {
    TranscriptItem::new(
        TranscriptItemId::new(format!("item-{index}")),
        spectacular_tui::Timestamp::new(index),
        content,
    )
}

/// Renders state through the active IOCraft application path.
fn render(state: &State) -> String {
    render_state_to_string(state, Some(100))
}

/// Returns the visible text from semantic render lines.
fn visible_text(lines: &[RenderLine]) -> Vec<String> {
    lines.iter().map(RenderLine::plain_text).collect()
}

#[test]
fn active_app_renders_terminal_flow_without_prototype_chrome() {
    let output = render(&state());

    assert!(output.contains("> "));
    assert!(output.contains("session-123 · /workspace/spectacular · GPT 5.1 (high)"));
    assert!(!output.contains("Transcript"));
    assert!(!output.contains("No transcript items yet"));
    assert!(!output.contains("Prompt:"));
    assert!(!output.contains("Completions:"));
    assert!(!output.contains("Guidance:"));
    assert!(!output.contains("Status: idle"));
    assert!(!output.contains("cwd:"));
    assert!(!output.contains("provider/model:"));
}

#[test]
fn opening_banner_matches_original_shape_width_and_styles() {
    let mut state = state();
    state.session.transcript.push(item(
        1,
        TranscriptItemContent::OpeningBanner(OpeningBannerItem::new(
            "0.1.0",
            "GPT 5.1",
            "high",
            "/workspace/spectacular",
            "session-123",
        )),
    ));

    let lines = app_render_lines(&state);
    let text = visible_text(&lines);
    let banner = &text[..7];

    assert_eq!(
        banner[0],
        "╭──────────────────────────────────────────────────────╮"
    );
    assert_eq!(
        banner[1],
        "│ Spectacular (v0.1.0)                                 │"
    );
    assert_eq!(
        banner[2],
        "│                                                      │"
    );
    assert_eq!(
        banner[3],
        "│ model:     GPT 5.1 high                              │"
    );
    assert_eq!(
        banner[4],
        "│ directory: /workspace/spectacular                    │"
    );
    assert_eq!(
        banner[5],
        "│ session:   session-123                               │"
    );
    assert_eq!(
        banner[6],
        "╰──────────────────────────────────────────────────────╯"
    );
    assert_eq!(lines[0].spans[0].style, RenderStyle::Title);
    assert_eq!(lines[1].spans[1].style, RenderStyle::Title);
    assert_eq!(lines[3].spans[1].style, RenderStyle::Text);
    assert_eq!(lines[4].spans[1].style, RenderStyle::Text);
    assert_eq!(lines[5].spans[1].style, RenderStyle::Text);
    assert_eq!(lines[6].spans[0].style, RenderStyle::Title);
}

#[test]
fn opening_banner_does_not_emit_mojibake() {
    let mut state = state();
    state.session.transcript.push(item(
        1,
        TranscriptItemContent::OpeningBanner(OpeningBannerItem::new(
            "0.1.0",
            "GPT 5.1",
            "high",
            "/workspace/spectacular",
            "session-123",
        )),
    ));

    let output = render(&state);

    assert!(output.contains('╭'));
    assert!(output.contains('─'));
    assert!(output.contains('│'));
    assert!(output.contains('╯'));
    assert!(!output.contains('�'));
}

#[test]
fn footer_includes_session_first_without_usage() {
    let state = state();
    let footer = footer_render_line(&state);

    assert_eq!(
        footer.plain_text(),
        "session-123 · /workspace/spectacular · GPT 5.1 (high)"
    );
    assert!(footer
        .spans
        .iter()
        .all(|span| span.style == RenderStyle::Dim));
}

#[test]
fn footer_omits_worktree_metadata_when_unavailable() {
    let state = State::new(SessionId::new("session-123"), runtime(), display(None));
    let footer = footer_render_line(&state);

    assert_eq!(
        footer.plain_text(),
        "session-123 · /workspace/spectacular · GPT 5.1 (high) · ~0/200k ctx"
    );
    assert!(footer
        .spans
        .iter()
        .all(|span| span.style == RenderStyle::Dim));
}

#[test]
fn footer_includes_clean_branch_worktree_metadata() {
    let mut display = display(None);
    display.worktree = Some(WorktreeMetadata::new("main"));
    let state = State::new(SessionId::new("session-123"), runtime(), display);

    assert_eq!(
        footer_render_line(&state).plain_text(),
        "session-123 · /workspace/spectacular · GPT 5.1 (high) · main · ~0/200k ctx"
    );
}

#[test]
fn footer_includes_dirty_branch_worktree_metadata() {
    let mut display = display(None);
    display.worktree = Some(WorktreeMetadata::new("main*"));
    let state = State::new(SessionId::new("session-123"), runtime(), display);

    assert_eq!(
        footer_render_line(&state).plain_text(),
        "session-123 · /workspace/spectacular · GPT 5.1 (high) · main* · ~0/200k ctx"
    );
}

#[test]
fn footer_includes_detached_head_worktree_metadata() {
    let mut display = display(None);
    display.worktree = Some(WorktreeMetadata::new("abc1234"));
    let state = State::new(SessionId::new("session-123"), runtime(), display);

    assert_eq!(
        footer_render_line(&state).plain_text(),
        "session-123 · /workspace/spectacular · GPT 5.1 (high) · abc1234 · ~0/200k ctx"
    );
}

#[test]
fn footer_includes_dirty_detached_head_worktree_metadata() {
    let mut display = display(None);
    display.worktree = Some(WorktreeMetadata::new("abc1234*"));
    let state = State::new(SessionId::new("session-123"), runtime(), display);

    assert_eq!(
        footer_render_line(&state).plain_text(),
        "session-123 · /workspace/spectacular · GPT 5.1 (high) · abc1234* · ~0/200k ctx"
    );
}

#[test]
fn footer_matches_original_shape_with_usage() {
    let context_usage = ContextTokenUsage::new(80_000, Some(200_000));
    let mut state = State::new(
        SessionId::new("session-123"),
        runtime(),
        display(Some(context_usage)),
    );
    state.session.context_usage = Some(context_usage);
    state.session.turn_usage = Some(TurnTokenUsage {
        input_tokens: 3_000,
        output_tokens: 9_000,
        total_tokens: 12_000,
        has_provider_metadata: true,
    });
    state.session.total_usage = Some(TurnTokenUsage {
        input_tokens: 30_000,
        output_tokens: 12_000,
        total_tokens: 42_000,
        has_provider_metadata: true,
    });

    let footer = footer_render_line(&state);

    assert_eq!(
        footer.plain_text(),
        "session-123 · /workspace/spectacular · GPT 5.1 (high) · 42k/200k tks"
    );
    assert_eq!(footer.spans.last().unwrap().style, RenderStyle::Dim);
}

#[test]
fn footer_usage_uses_warning_and_critical_styles() {
    let warning = State::new(
        SessionId::new("session-123"),
        runtime(),
        display(Some(ContextTokenUsage::new(160, Some(200)))),
    );
    let critical = State::new(
        SessionId::new("session-123"),
        runtime(),
        display(Some(ContextTokenUsage::new(180, Some(200)))),
    );

    assert_eq!(
        footer_right_render_line(&warning).unwrap().spans[0].style,
        RenderStyle::Warning
    );
    assert_eq!(
        footer_right_render_line(&critical).unwrap().spans[0].style,
        RenderStyle::Error
    );
}

#[test]
fn prompt_rows_use_original_marker_and_continuation_indentation() {
    let mut state = state();
    state.session.prompt = spectacular_tui::PromptState::from_text("first\nsecond");

    let text = visible_text(&app_render_lines(&state));

    assert!(text.contains(&"> first".to_string()));
    assert!(text.contains(&"  second".to_string()));
}

#[test]
fn historical_user_prompt_matches_original_submitted_prompt_shape() {
    let mut state = state();
    state.session.transcript.push(item(
        1,
        TranscriptItemContent::UserPrompt(spectacular_tui::UserPromptItem::new("hello\nthere")),
    ));

    let text = visible_text(&app_render_lines(&state));

    assert!(text.contains(&"> hello".to_string()));
    assert!(text.contains(&"  there".to_string()));
}

#[test]
fn semantic_event_rows_use_original_casing_and_prefixes() {
    let mut state = state();
    state.session.transcript = vec![
        item(
            1,
            TranscriptItemContent::AssistantMessage(spectacular_tui::AssistantMessageItem::new(
                "hi there",
            )),
        ),
        item(
            2,
            TranscriptItemContent::Warning(spectacular_tui::WarningItem::new("careful")),
        ),
        item(
            3,
            TranscriptItemContent::Error(spectacular_tui::ErrorItem::new(
                "boom",
                Some("details".to_string()),
            )),
        ),
        item(
            4,
            TranscriptItemContent::Success(spectacular_tui::SuccessItem::new("done")),
        ),
        item(
            5,
            TranscriptItemContent::Notice(spectacular_tui::NoticeItem::new("plain notice")),
        ),
        item(
            6,
            TranscriptItemContent::Cancellation(spectacular_tui::CancellationItem::new(
                "user stopped run",
            )),
        ),
        item(
            7,
            TranscriptItemContent::ToolCall(spectacular_tui::ToolCallItem {
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
            TranscriptItemContent::Command(spectacular_tui::CommandItem {
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
fn working_line_matches_original_shape() {
    let mut state = state();
    state.status = Status::Running {
        activity: spectacular_tui::Activity::WaitingForModel,
        cancellable: true,
    };

    let output = render(&state);

    assert!(output.contains("⠙ Working (CTRL + C to stop)"));
    assert!(!output.contains('�'));
}

#[test]
fn worked_summary_matches_original_shape() {
    let mut state = state();
    state.session.transcript.push(item(
        1,
        TranscriptItemContent::WorkedSummary(spectacular_tui::WorkedSummaryItem::new(
            "3s",
            Some(77),
        )),
    ));

    let output = render(&state);

    assert!(output.contains("Worked for 3s · total 77 tokens"));
    assert!(!output.contains('�'));
}

#[test]
fn active_render_applies_semantic_styles() {
    let mut state = state();
    state.session.transcript = vec![
        item(
            1,
            TranscriptItemContent::UserPrompt(spectacular_tui::UserPromptItem::new("hello")),
        ),
        item(
            2,
            TranscriptItemContent::AssistantMessage(spectacular_tui::AssistantMessageItem::new(
                "hi",
            )),
        ),
        item(
            3,
            TranscriptItemContent::Warning(spectacular_tui::WarningItem::new("careful")),
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
