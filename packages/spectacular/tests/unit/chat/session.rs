    use super::*;

    #[test]
    fn new_session_schema_version_is_v2() {
        let event = store::session_started("a83f19c2", SCHEMA_VERSION, UNTITLED);

        assert!(matches!(
            event,
            ChatEvent::SessionStarted {
                schema_version: 2,
                ..
            }
        ));
    }
