    use super::*;

    #[test]
    fn summary_counts_conversation_turns_and_groups_assistant_deltas() {
        let records = vec![
            known(ChatEvent::SessionStarted {
                schema_version: 1,
                id: "a83f19c2".to_owned(),
                title: UNTITLED.to_owned(),
                created_at: "2026-04-29T14:00:00Z".to_owned(),
            }),
            known(ChatEvent::UserPrompt {
                id: None,
                content: "hello".to_owned(),
                created_at: "2026-04-29T14:01:00Z".to_owned(),
            }),
            known(ChatEvent::AssistantDelta {
                role: "assistant".to_owned(),
                id: "message-1".to_owned(),
                content: "one".to_owned(),
                created_at: "2026-04-29T14:02:00Z".to_owned(),
            }),
            known(ChatEvent::AssistantDelta {
                role: "assistant".to_owned(),
                id: "message-1".to_owned(),
                content: " two".to_owned(),
                created_at: "2026-04-29T14:03:00Z".to_owned(),
            }),
            known(ChatEvent::Finished {
                reason: "stop".to_owned(),
                created_at: "2026-04-29T14:04:00Z".to_owned(),
            }),
        ];

        let summary = summarize("a83f19c2", &records);

        assert_eq!(summary.messages, 2);
        assert_eq!(summary.updated.to_rfc3339(), "2026-04-29T14:04:00+00:00");
    }

    #[test]
    fn summary_uses_latest_title_event() {
        let records = vec![
            known(ChatEvent::SessionTitleUpdated {
                title: "Old".to_owned(),
                slot: "labeling".to_owned(),
                model: "label/model".to_owned(),
                source: None,
                created_at: "2026-04-29T14:00:00Z".to_owned(),
            }),
            known(ChatEvent::SessionTitleUpdated {
                title: "New Title".to_owned(),
                slot: "labeling".to_owned(),
                model: "label/model".to_owned(),
                source: None,
                created_at: "2026-04-29T14:01:00Z".to_owned(),
            }),
            known(ChatEvent::UserPrompt {
                id: None,
                content: "hello".to_owned(),
                created_at: "2026-04-29T14:02:00Z".to_owned(),
            }),
        ];

        let summary = summarize("a83f19c2", &records);

        assert_eq!(summary.title, "New Title");
    }

    #[test]
    fn matching_ids_ignores_snapshot_files() {
        let dir = temp_dir("matching_ids_ignores_snapshot_files");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a83f19c2.jsonl"), "").unwrap();
        fs::write(dir.join("a83f19c2.snapshot.json"), "{}").unwrap();
        fs::write(dir.join("a83f19c3.snapshot.json"), "{}").unwrap();

        let matches = SessionIndex::new(&dir).matching_ids("a83f19c").unwrap();

        assert_eq!(matches, vec!["a83f19c2"]);

        let _ = fs::remove_dir_all(dir);
    }

    /// Creates a unique temp directory for session index tests.
    fn temp_dir(test_name: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("spectacular-session-index-{test_name}-{nanos}"))
    }

    fn known(event: ChatEvent) -> ChatRecord {
        ChatRecord::Known { line: 1, event }
    }
