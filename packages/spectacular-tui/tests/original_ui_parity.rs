use iocraft::prelude::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, TerminalEvent};
use spectacular_tui::{
    reduce, render_state_to_string, tui_event_effects, CancellationItem, ChatTuiAction,
    CommandDescriptor, CommandStatus, ContextTokenUsage, DisplayMetadata, EventEffect,
    OpeningBannerItem, PromptState, ReasoningLevel, RuntimeSelection, SessionId, State,
    SuccessItem, ToolStatus, TranscriptItem, TranscriptItemContent, TranscriptItemId, WarningItem,
    WorkedSummaryItem,
};

fn runtime() -> RuntimeSelection {
    RuntimeSelection::new(
        "openai-compatible",
        "openrouter",
        "gpt-5.1",
        ReasoningLevel::High,
        None,
    )
}

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

fn state() -> State {
    State::new(SessionId::new("session-123"), runtime(), display(None))
}

fn item(index: u64, content: TranscriptItemContent) -> TranscriptItem {
    TranscriptItem::new(
        TranscriptItemId::new(format!("item-{index}")),
        spectacular_tui::Timestamp::new(index),
        content,
    )
}

fn render(state: &State) -> String {
    render_state_to_string(state, Some(100))
}

fn key(code: KeyCode, modifiers: KeyModifiers) -> TerminalEvent {
    let mut event = KeyEvent::new(KeyEventKind::Press, code);
    event.modifiers = modifiers;
    TerminalEvent::Key(event)
}

#[test]
fn opening_banner_empty_transcript_prompt_and_footer_match_original_shape() {
    let usage = ContextTokenUsage::new(42_000, Some(200_000));
    let mut state = State::new(
        SessionId::new("session-123"),
        runtime(),
        display(Some(usage)),
    );
    state.session.usage = Some(usage);
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

    assert!(output.contains("Spectacular (v0.1.0)"));
    assert!(output.contains("model:     GPT 5.1 high"));
    assert!(output.contains("directory: /workspace/spectacular"));
    assert!(output.contains("session:   session-123"));
    assert!(output.contains(">"));
    assert!(output.contains("/workspace/spectacular · GPT 5.1 (high) · 42k/200k tks"));
    assert!(!output.contains("Transcript"));
    assert!(!output.contains("No transcript items yet"));
    assert!(!output.contains("Status:"));
    assert!(!output.contains("Prompt:"));
    assert!(!output.contains("Completions:"));
    assert!(!output.contains("Guidance:"));
    assert!(!output.contains("cwd:"));
    assert!(!output.contains("provider/model:"));
}

#[test]
fn transcript_semantic_items_render_without_prototype_labels() {
    let mut state = state();
    state.session.transcript = vec![
        item(
            1,
            TranscriptItemContent::UserPrompt(spectacular_tui::UserPromptItem::new("hello")),
        ),
        item(
            2,
            TranscriptItemContent::AssistantMessage(spectacular_tui::AssistantMessageItem::new(
                "hi there",
            )),
        ),
        item(
            3,
            TranscriptItemContent::Reasoning(spectacular_tui::ReasoningItem::new(
                "thinking", false,
            )),
        ),
        item(
            4,
            TranscriptItemContent::Warning(WarningItem::new("careful")),
        ),
        item(5, TranscriptItemContent::Success(SuccessItem::new("done"))),
        item(
            6,
            TranscriptItemContent::Cancellation(CancellationItem::new("user stopped run")),
        ),
        item(
            7,
            TranscriptItemContent::Error(spectacular_tui::ErrorItem::new(
                "boom",
                Some("details".to_string()),
            )),
        ),
        item(
            8,
            TranscriptItemContent::Notice(spectacular_tui::NoticeItem::new("plain notice")),
        ),
        item(
            9,
            TranscriptItemContent::WorkedSummary(WorkedSummaryItem::new("3s", Some(77))),
        ),
        item(
            10,
            TranscriptItemContent::ToolCall(spectacular_tui::ToolCallItem {
                tool_call_id: "tool-1".to_string(),
                name: "grep".to_string(),
                arguments_preview: Some("pattern".to_string()),
                status: ToolStatus::Finished,
                output_preview: Some("Edited file\n1 - old\n2 + new".to_string()),
            }),
        ),
        item(
            11,
            TranscriptItemContent::Command(spectacular_tui::CommandItem {
                command_id: "cmd-1".to_string(),
                command: "cargo test".to_string(),
                status: CommandStatus::Failed,
                output: "failure output".to_string(),
                exit_code: Some(101),
            }),
        ),
    ];

    let output = render(&state);

    assert!(output.contains("hello"));
    assert!(output.contains("hi there"));
    assert!(output.contains("thinking"));
    assert!(output.contains("warning: careful"));
    assert!(output.contains("done"));
    assert!(output.contains("user stopped run"));
    assert!(output.contains("error: boom"));
    assert!(output.contains("details"));
    assert!(output.contains("plain notice"));
    assert!(output.contains("Worked for 3s · total 77 tokens"));
    assert!(output.contains("grep pattern"));
    assert!(output.contains("Edited file"));
    assert!(output.contains("$ cargo test"));
    assert!(output.contains("failure output"));
    assert!(!output.contains("You:"));
    assert!(!output.contains("Assistant:"));
    assert!(!output.contains("Reasoning:"));
    assert!(!output.contains("Tool:"));
    assert!(!output.contains("Command:"));
    assert!(!output.contains("Notice:"));
    assert!(!output.contains("Error:"));
}

#[test]
fn prompt_multiline_and_slash_suggestions_match_original_shape() {
    let mut state = state();
    state.commands = vec![
        CommandDescriptor::with_usage("config", "Manage configuration", "/config list"),
        CommandDescriptor::new("session", "Manage sessions"),
    ];
    state.session.prompt = PromptState::from_text("/c");

    let output = render(&state);

    assert!(output.contains("> /c"));
    assert!(output.contains("  /config            Manage configuration"));
    assert!(!output.contains("/session"));

    state.session.prompt = PromptState::from_text("first\nsecond");
    let output = render(&state);

    assert!(output.contains("> first"));
    assert!(output.contains("  second"));
}

#[test]
fn working_spinner_and_completed_summary_match_original_text() {
    let mut state = state();
    reduce(&mut state, ChatTuiAction::AgentStarted);

    let output = render(&state);

    assert!(output.contains("⠙ Working (CTRL + C to stop)"));
    assert!(!output.contains("Status: running"));

    reduce(
        &mut state,
        ChatTuiAction::WorkedSummaryReported {
            duration: "3s".to_string(),
            turn_tokens: Some(77),
        },
    );
    reduce(&mut state, ChatTuiAction::AgentFinished);
    let output = render(&state);

    assert!(output.contains("Worked for 3s · total 77 tokens"));
}

#[test]
fn idle_ctrl_c_clears_non_empty_prompt_before_requesting_exit() {
    let mut state = state();
    state.session.prompt = PromptState::from_text("draft");

    let effects = tui_event_effects(&state, key(KeyCode::Char('c'), KeyModifiers::CONTROL));

    assert_eq!(
        effects,
        vec![EventEffect::Action(ChatTuiAction::PromptChanged(
            PromptState::empty()
        ))]
    );

    reduce(
        &mut state,
        match effects.into_iter().next().unwrap() {
            EventEffect::Action(action) => action,
            EventEffect::RequestExit => panic!("expected prompt clear"),
        },
    );
    let effects = tui_event_effects(&state, key(KeyCode::Char('c'), KeyModifiers::CONTROL));

    assert_eq!(effects, vec![EventEffect::RequestExit]);
}
