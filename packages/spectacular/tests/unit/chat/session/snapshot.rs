    use super::*;
    use spectacular_tui::{
        CommandItem, CommandStatus, ContextTokenUsage, ErrorItem, NoticeItem, PromptState,
        ReasoningItem, Session, SessionId, Timestamp, ToolCallItem, ToolStatus, TranscriptItem,
        TranscriptItemContent, TranscriptItemId, UserPromptItem, AssistantMessageItem,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Builds a stable semantic transcript item for snapshot store tests.
    fn item(id: &str, timestamp: u64, content: TranscriptItemContent) -> TranscriptItem {
        TranscriptItem::new(TranscriptItemId::new(id), Timestamp::new(timestamp), content)
    }

    /// Builds a durable semantic session snapshot with all persisted transcript variants.
    fn snapshot() -> Session {
        Session {
            id: SessionId::new("session-1"),
            transcript: vec![
                item(
                    "user-1",
                    1,
                    TranscriptItemContent::UserPrompt(UserPromptItem::new("hello")),
                ),
                item(
                    "assistant-1",
                    2,
                    TranscriptItemContent::AssistantMessage(AssistantMessageItem::new("hi")),
                ),
                item(
                    "reasoning-1",
                    3,
                    TranscriptItemContent::Reasoning(ReasoningItem::new("thinking", false)),
                ),
                item(
                    "tool-1",
                    4,
                    TranscriptItemContent::ToolCall(ToolCallItem {
                        tool_call_id: "call-1".to_owned(),
                        name: "grep".to_owned(),
                        arguments_preview: Some("pattern".to_owned()),
                        status: ToolStatus::Failed,
                        output_preview: Some("no matches".to_owned()),
                    }),
                ),
                item(
                    "command-1",
                    5,
                    TranscriptItemContent::Command(CommandItem {
                        command_id: "cmd-1".to_owned(),
                        command: "cargo test".to_owned(),
                        status: CommandStatus::Finished,
                        output: "ok\n".to_owned(),
                        exit_code: Some(0),
                    }),
                ),
                item(
                    "error-1",
                    6,
                    TranscriptItemContent::Error(ErrorItem::new("boom", Some("details".to_owned()))),
                ),
                item(
                    "notice-1",
                    7,
                    TranscriptItemContent::Notice(NoticeItem::new("heads up")),
                ),
            ],
            next_timestamp: Timestamp::new(8),
            prompt: PromptState::from_text("draft"),
            usage: Some(ContextTokenUsage::new(70, Some(100))),
        }
    }

    /// Verifies the session store saves and loads the semantic TUI snapshot unchanged.
    #[test]
    fn save_and_load_session_snapshot_round_trips_semantic_state() {
        let dir = temp_dir("save_and_load_session_snapshot_round_trips_semantic_state");
        let store = SessionStore::new(dir.clone()).unwrap();
        let expected = snapshot();

        store.save_snapshot(&expected).unwrap();
        let restored = store.load_snapshot(expected.id.as_str()).unwrap();

        assert_eq!(restored, expected);
        assert!(matches!(
            &restored.transcript[3].content,
            TranscriptItemContent::ToolCall(tool)
                if restored.transcript[3].id == TranscriptItemId::new("tool-1")
                    && tool.tool_call_id == "call-1"
                    && tool.status == ToolStatus::Failed
                    && tool.output_preview.as_deref() == Some("no matches")
        ));
        assert!(matches!(
            &restored.transcript[4].content,
            TranscriptItemContent::Command(command)
                if restored.transcript[4].id == TranscriptItemId::new("command-1")
                    && command.command_id == "cmd-1"
                    && command.status == CommandStatus::Finished
                    && command.output == "ok\n"
                    && command.exit_code == Some(0)
        ));

        let _ = std::fs::remove_dir_all(dir);
    }

    /// Verifies semantic snapshots are stored separately from append-only JSONL events.
    #[test]
    fn snapshot_path_does_not_use_jsonl_event_log() {
        let dir = temp_dir("snapshot_path_does_not_use_jsonl_event_log");
        let store = SessionStore::new(dir.clone()).unwrap();
        let session = snapshot();

        store.save_snapshot(&session).unwrap();

        assert!(store.snapshot_path(session.id.as_str()).exists());
        assert!(!store.path(session.id.as_str()).exists());

        let _ = std::fs::remove_dir_all(dir);
    }

    /// Creates a unique temp directory for session snapshot store tests.
    fn temp_dir(test_name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("spectacular-session-snapshot-{test_name}-{nanos}"))
    }
