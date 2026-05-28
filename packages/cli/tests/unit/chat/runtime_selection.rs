    use super::*;
    use ::config::{
        CachedModelMetadata, ModelCache, ModelConfig, ProviderConfig, TaskAssignments,
    };
    use std::collections::BTreeMap;

    /// Verifies that runtime selection uses latest session provider and model events.
    #[test]
    fn runtime_selection_uses_latest_session_provider_and_model_events() {
        let records = vec![
            chat_record(ChatEvent::ProviderChanged {
                provider: "openrouter".to_owned(),
                source: Some("global_default".to_owned()),
                created_at: "2026-04-29T14:00:00Z".to_owned(),
            }),
            chat_record(ChatEvent::ModelChanged {
                slot: "coding".to_owned(),
                provider: "openrouter".to_owned(),
                model: "old/model".to_owned(),
                reasoning: "low".to_owned(),
                source: Some("global_default".to_owned()),
                created_at: "2026-04-29T14:00:00Z".to_owned(),
            }),
            chat_record(ChatEvent::ModelChanged {
                slot: "coding".to_owned(),
                provider: "openrouter".to_owned(),
                model: "new/model".to_owned(),
                reasoning: "high".to_owned(),
                source: Some("command".to_owned()),
                created_at: "2026-04-29T14:01:00Z".to_owned(),
            }),
        ];

        let runtime = RuntimeSelection::from_session_records_and_cache(
            &complete_config(),
            &ModelCache::default(),
            &records,
        )
        .unwrap()
        .unwrap();

        assert_eq!(runtime.provider, "openrouter");
        assert_eq!(runtime.api_key, "sk-or-v1-test");
        assert_eq!(runtime.model, "new/model");
        assert_eq!(runtime.reasoning, ReasoningLevel::High);
    }

    /// Verifies that runtime selection falls back when session provider is unavailable.
    #[test]
    fn runtime_selection_falls_back_when_session_provider_is_unavailable() {
        let records = vec![
            chat_record(ChatEvent::ProviderChanged {
                provider: "missing".to_owned(),
                source: Some("global_default".to_owned()),
                created_at: "2026-04-29T14:00:00Z".to_owned(),
            }),
            chat_record(ChatEvent::ModelChanged {
                slot: "coding".to_owned(),
                provider: "missing".to_owned(),
                model: "missing/model".to_owned(),
                reasoning: "medium".to_owned(),
                source: Some("global_default".to_owned()),
                created_at: "2026-04-29T14:00:00Z".to_owned(),
            }),
        ];

        let runtime = RuntimeSelection::from_session_records_and_cache(
            &complete_config(),
            &ModelCache::default(),
            &records,
        )
        .unwrap();

        assert!(runtime.is_none());
    }

    /// Verifies that runtime selection uses cached context window metadata.
    #[test]
    fn runtime_selection_uses_cached_context_window_metadata() {
        let mut cache = ModelCache::default();
        cache.put_provider(
            "openrouter",
            "openrouter",
            42,
            [CachedModelMetadata::new("global/coding", "Global Coding", Vec::<String>::new())
                .with_context_window_tokens(Some(64_000))],
        );

        let runtime = RuntimeSelection::from_config_and_cache(&complete_config(), &cache).unwrap();

        assert_eq!(runtime.context_window_tokens, Some(64_000));
    }

    /// Builds a complete configuration for test scenarios.
    fn complete_config() -> DoricConfig {
        let mut providers = BTreeMap::new();
        providers.insert(
            "openrouter".to_owned(),
            ProviderConfig::new("openrouter", "sk-or-v1-test"),
        );
        let mut models = BTreeMap::new();
        models.insert(
            "global-coding".to_owned(),
            ModelConfig::new("openrouter", "global/coding", ReasoningLevel::Medium),
        );

        DoricConfig {
            providers,
            models,
            tasks: TaskAssignments {
                general: None,
                coding: Some("global-coding".to_owned()),
                labeling: None,
            },
        }
    }

    /// Wraps a chat event in a session record for runtime-selection tests.
    fn chat_record(event: ChatEvent) -> session::ChatRecord {
        session::ChatRecord::Known { line: 1, event }
    }
