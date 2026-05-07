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
                content: "hello".to_owned(),
                created_at: "2026-04-29T14:01:00Z".to_owned(),
            }),
            known(ChatEvent::AssistantDelta {
                role: "assistant".to_owned(),
                content: "one".to_owned(),
                created_at: "2026-04-29T14:02:00Z".to_owned(),
            }),
            known(ChatEvent::AssistantDelta {
                role: "assistant".to_owned(),
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
                content: "hello".to_owned(),
                created_at: "2026-04-29T14:02:00Z".to_owned(),
            }),
        ];

        let summary = summarize("a83f19c2", &records);

        assert_eq!(summary.title, "New Title");
    }

    fn known(event: ChatEvent) -> ChatRecord {
        ChatRecord::Known { line: 1, event }
    }
