use spectacular_tui::{
    render_state_to_string, Activity, CommandStatus, ContextTokenUsage, DisplayMetadata,
    ReasoningLevel, RuntimeSelection, SessionId, State, Status, ToolStatus, TranscriptItem,
    TranscriptItemContent, TranscriptItemId,
};

/// Builds a representative runtime selection for IOCraft layout tests.
fn runtime() -> RuntimeSelection {
    RuntimeSelection::new(
        "openai-compatible",
        "openrouter",
        "gpt-5.1",
        ReasoningLevel::High,
        Some(200_000),
    )
}

/// Builds display metadata for IOCraft layout tests.
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
    State::new(SessionId::new("session-123"), runtime(), display(None))
}

/// Creates a transcript item with stable test identity and timestamp.
fn item(index: u64, content: TranscriptItemContent) -> TranscriptItem {
    TranscriptItem::new(
        TranscriptItemId::new(format!("item-{index}")),
        spectacular_tui::Timestamp::new(index),
        content,
    )
}

/// Renders state through the IOCraft prototype and returns plain text output.
fn render(state: &State) -> String {
    render_state_to_string(state, Some(100))
}

/// Verifies the empty read-only layout renders all major regions.
#[test]
fn empty_state_renders_full_layout_regions() {
    let output = render(&state());

    assert!(output.contains("Spectacular"));
    assert!(output.contains("Transcript"));
    assert!(output.contains("No transcript items yet"));
    assert!(output.contains("Status: idle"));
    assert!(output.contains("Prompt"));
    assert!(output.contains("Completions: reserved"));
    assert!(output.contains("Guidance: reserved"));
    assert!(output.contains("cwd: /workspace/spectacular"));
}

/// Verifies every semantic transcript item kind renders without ANSI pre-rendered strings.
#[test]
fn populated_transcript_renders_all_semantic_item_kinds() {
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
            TranscriptItemContent::ToolCall(spectacular_tui::ToolCallItem {
                tool_call_id: "tool-1".to_string(),
                name: "grep".to_string(),
                arguments_preview: Some("pattern".to_string()),
                status: ToolStatus::Finished,
                output_preview: Some("match".to_string()),
            }),
        ),
        item(
            5,
            TranscriptItemContent::Command(spectacular_tui::CommandItem {
                command_id: "cmd-1".to_string(),
                command: "cargo test".to_string(),
                status: CommandStatus::Failed,
                output: "failure output".to_string(),
                exit_code: Some(101),
            }),
        ),
        item(
            6,
            TranscriptItemContent::Error(spectacular_tui::ErrorItem::new(
                "boom",
                Some("details".to_string()),
            )),
        ),
        item(
            7,
            TranscriptItemContent::Notice(spectacular_tui::NoticeItem::new(
                "Welcome to Spectacular",
            )),
        ),
    ];

    let output = render(&state);

    assert!(output.contains("You: hello"));
    assert!(output.contains("Assistant: hi there"));
    assert!(output.contains("Reasoning: thinking"));
    assert!(output.contains("Tool: grep [finished]"));
    assert!(output.contains("args: pattern"));
    assert!(output.contains("output: match"));
    assert!(output.contains("Command: cargo test [failed]"));
    assert!(output.contains("exit: 101"));
    assert!(output.contains("Error: boom"));
    assert!(output.contains("details"));
    assert!(output.contains("Notice: Welcome to Spectacular"));
}

/// Verifies header metadata is read from state rather than external services.
#[test]
fn header_renders_state_metadata() {
    let output = render(&state());

    assert!(output.contains("model: GPT 5.1"));
    assert!(output.contains("reasoning: high"));
    assert!(output.contains("directory: /workspace/spectacular"));
    assert!(output.contains("session: session-123"));
}

/// Verifies running status includes activity and current spinner frame.
#[test]
fn running_status_renders_activity_and_spinner() {
    let mut state = state();
    state.status = Status::Running {
        activity: Activity::RunningTool {
            id: TranscriptItemId::new("item-4"),
            name: "grep".to_string(),
        },
        cancellable: true,
    };

    let output = render(&state);

    assert!(output.contains("Status: running"));
    assert!(output.contains("activity: running tool grep"));
    assert!(output.contains(state.spinner.current_frame()));
}

/// Verifies context usage renders in both status and footer regions when available.
#[test]
fn usage_renders_in_status_and_footer() {
    let usage = ContextTokenUsage::new(42_000, Some(200_000));
    let mut state = State::new(
        SessionId::new("session-123"),
        runtime(),
        display(Some(usage)),
    );
    state.session.usage = Some(usage);

    let output = render(&state);

    assert!(output.contains("tokens: 42000/200000"));
    assert!(output.contains("context: 42000/200000"));
}

/// Verifies non-command prompt text keeps reserved completion and guidance regions.
#[test]
fn prompt_placeholder_renders_current_text_and_reserved_regions() {
    let mut state = state();
    state.session.prompt = spectacular_tui::PromptState::from_text("model gpt-5.1");

    let output = render(&state);

    assert!(output.contains("Prompt: model gpt-5.1"));
    assert!(output.contains("Completions: reserved"));
    assert!(output.contains("Guidance: reserved"));
}

/// Verifies welcome/banner text is rendered as semantic state instead of terminal printing.
#[test]
fn opening_banner_renders_from_semantic_transcript_state() {
    let mut state = state();
    state.session.transcript.push(item(
        1,
        TranscriptItemContent::Notice(spectacular_tui::NoticeItem::new("Welcome to Spectacular")),
    ));

    let output = render(&state);

    assert!(output.contains("Notice: Welcome to Spectacular"));
}

/// Verifies rendering is a pure state projection with no runtime side effects.
#[test]
fn rendering_does_not_mutate_state_or_require_side_effects() {
    let state = state();
    let original = state.clone();

    let output = render(&state);

    assert!(output.contains("Spectacular"));
    assert_eq!(state, original);
}
