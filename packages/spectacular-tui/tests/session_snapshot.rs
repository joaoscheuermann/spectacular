use spectacular_tui::{
    AssistantMessageItem, CommandDescriptor, CommandItem, CommandStatus, ContextTokenUsage,
    DisplayMetadata, ErrorItem, NoticeItem, PromptState, ReasoningItem, ReasoningLevel,
    RuntimeSelection, Session, SessionId, State, Status, Timestamp, ToolCallItem, ToolStatus,
    TranscriptItem, TranscriptItemContent, TranscriptItemId, UserPromptItem,
};

/// Builds a stable transcript item with semantic content for snapshot tests.
fn transcript_item(id: &str, timestamp: u64, content: TranscriptItemContent) -> TranscriptItem {
    TranscriptItem::new(
        TranscriptItemId::new(id),
        Timestamp::new(timestamp),
        content,
    )
}

/// Builds a representative runtime selection for state reconstruction tests.
fn runtime() -> RuntimeSelection {
    RuntimeSelection::new(
        "openai-compatible",
        "provider",
        "model",
        ReasoningLevel::High,
        Some(128_000),
    )
}

/// Builds visible display metadata for state reconstruction tests.
fn display() -> DisplayMetadata {
    DisplayMetadata::new("provider", "model", "high", "/workspace", "session-1", None)
}

/// Builds a durable session snapshot containing every semantic transcript variant.
fn session_snapshot() -> Session {
    Session {
        id: SessionId::new("session-1"),
        transcript: vec![
            transcript_item(
                "user-1",
                1,
                TranscriptItemContent::UserPrompt(UserPromptItem::new("hello")),
            ),
            transcript_item(
                "assistant-1",
                2,
                TranscriptItemContent::AssistantMessage(AssistantMessageItem::new("hi")),
            ),
            transcript_item(
                "reasoning-1",
                3,
                TranscriptItemContent::Reasoning(ReasoningItem::new("thinking", true)),
            ),
            transcript_item(
                "tool-1",
                4,
                TranscriptItemContent::ToolCall(ToolCallItem {
                    tool_call_id: "call-1".to_owned(),
                    name: "grep".to_owned(),
                    arguments_preview: Some("State".to_owned()),
                    status: ToolStatus::Finished,
                    output_preview: Some("match".to_owned()),
                    display: None,
                }),
            ),
            transcript_item(
                "command-1",
                5,
                TranscriptItemContent::Command(CommandItem {
                    command_id: "cmd-1".to_owned(),
                    command: "cargo test".to_owned(),
                    status: CommandStatus::Failed,
                    output: "failed\n".to_owned(),
                    exit_code: Some(101),
                    display: None,
                }),
            ),
            transcript_item(
                "error-1",
                6,
                TranscriptItemContent::Error(ErrorItem::new("boom", Some("details".to_owned()))),
            ),
            transcript_item(
                "notice-1",
                7,
                TranscriptItemContent::Notice(NoticeItem::new("cancelled")),
            ),
        ],
        next_timestamp: Timestamp::new(8),
        prompt: PromptState::from_text("draft"),
        usage: Some(ContextTokenUsage::new(55, Some(128_000))),
    }
}

/// Verifies semantic session snapshots round-trip without losing durable lifecycle data.
#[test]
fn session_snapshot_serializes_and_deserializes_durable_transcript_data() {
    let session = session_snapshot();

    let value = serde_json::to_value(&session).unwrap();
    assert!(value.get("next_timestamp").is_none());
    assert!(value.get("commands").is_none());
    assert!(value.get("runtime").is_none());
    assert!(value.get("display").is_none());
    assert!(value.get("status").is_none());
    assert!(value.get("spinner").is_none());
    assert!(value.get("selection").is_none());
    assert!(value.get("scroll").is_none());

    let restored: Session = serde_json::from_value(value).unwrap();

    assert_eq!(restored, session);
    assert!(matches!(
        &restored.transcript[3].content,
        TranscriptItemContent::ToolCall(item)
            if restored.transcript[3].id == TranscriptItemId::new("tool-1")
                && item.tool_call_id == "call-1"
                && item.status == ToolStatus::Finished
                && item.output_preview.as_deref() == Some("match")
    ));
    assert!(matches!(
        &restored.transcript[4].content,
        TranscriptItemContent::Command(item)
            if restored.transcript[4].id == TranscriptItemId::new("command-1")
                && item.command_id == "cmd-1"
                && item.status == CommandStatus::Failed
                && item.output == "failed\n"
                && item.exit_code == Some(101)
    ));
    assert_eq!(restored.prompt, PromptState::from_text("draft"));
    assert_eq!(
        restored.usage,
        Some(ContextTokenUsage::new(55, Some(128_000)))
    );
}

/// Verifies replay reconstructs live state from durable session data and fresh metadata.
#[test]
fn state_reconstruction_initializes_transient_fields_from_defaults() {
    let session = session_snapshot();
    let commands = vec![CommandDescriptor::new("help", "Show help")];
    let runtime = runtime();
    let display = display();

    let state = State::from_session(
        session.clone(),
        commands.clone(),
        runtime.clone(),
        display.clone(),
    );

    assert_eq!(state.session, session);
    assert_eq!(state.commands, commands);
    assert_eq!(state.runtime, runtime);
    assert_eq!(state.display.usage, session.usage);
    assert_eq!(state.display.provider_label, display.provider_label);
    assert_eq!(state.status, Status::Idle);
    assert_eq!(state.spinner, Default::default());
    assert_eq!(state.selection, None);
    assert_eq!(state.scroll, Default::default());
    assert!(state.scroll.follow_tail);
}

/// Verifies replay keeps prior ANSI output as semantic data instead of replaying terminal writes.
#[test]
fn state_reconstruction_does_not_replay_terminal_output() {
    let mut session = session_snapshot();
    session.transcript.push(transcript_item(
        "command-ansi",
        8,
        TranscriptItemContent::Command(CommandItem {
            command_id: "cmd-ansi".to_owned(),
            command: "printf red".to_owned(),
            status: CommandStatus::Finished,
            output: "\u{1b}[31mred\u{1b}[0m".to_owned(),
            exit_code: Some(0),
            display: None,
        }),
    ));
    session.next_timestamp = Timestamp::new(9);

    let state = State::from_session(session, Vec::new(), runtime(), display());

    assert_eq!(state.status, Status::Idle);
    assert!(matches!(
        &state.session.transcript.last().unwrap().content,
        TranscriptItemContent::Command(item)
            if item.output == "\u{1b}[31mred\u{1b}[0m"
                && item.status == CommandStatus::Finished
    ));
}
