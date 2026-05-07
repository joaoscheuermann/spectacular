    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn read_preserves_corrupt_and_unknown_records() {
        let dir = temp_dir("read_preserves_corrupt_and_unknown_records");
        let store = SessionStore::new(dir.clone()).unwrap();
        let path = store.path("a83f19c2");
        fs::write(
            &path,
            concat!(
                "{\"type\":\"user_prompt\",\"content\":\"hello\",\"created_at\":\"2026-04-29T14:01:00Z\"}\n",
                "{\"type\":\"future_event\",\"payload\":true}\n",
                "not json\n"
            ),
        )
        .unwrap();

        let records = store.read(&path).unwrap();

        assert!(matches!(records[0], ChatRecord::Known { .. }));
        assert!(matches!(records[1], ChatRecord::Unknown { .. }));
        assert!(matches!(records[2], ChatRecord::Corrupt { .. }));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn truncate_after_latest_user_prompt_keeps_prompt_and_drops_later_events() {
        let dir = temp_dir("truncate_after_latest_user_prompt");
        let store = SessionStore::new(dir.clone()).unwrap();
        let path = store.path("a83f19c2");
        fs::write(
            &path,
            concat!(
                "{\"type\":\"user_prompt\",\"content\":\"hello\",\"created_at\":\"2026-04-29T14:01:00Z\"}\n",
                "{\"type\":\"assistant_delta\",\"role\":\"assistant\",\"content\":\"hi\",\"created_at\":\"2026-04-29T14:02:00Z\"}\n"
            ),
        )
        .unwrap();

        let prompt = store.truncate_after_latest_user_prompt(&path).unwrap();
        let content = fs::read_to_string(&path).unwrap();

        assert_eq!(prompt, "hello");
        assert!(content.contains("\"user_prompt\""));
        assert!(!content.contains("\"assistant_delta\""));

        let _ = fs::remove_dir_all(dir);
    }

    fn temp_dir(test_name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("spectacular-chat-{test_name}-{nanos}"))
    }
