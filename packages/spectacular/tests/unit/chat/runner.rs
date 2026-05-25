    use super::*;
    use spectacular_config::ReasoningLevel;
    use spectacular_llms::{OpenRouterProvider, OPENROUTER_PROVIDER_ID};
    use spectacular_tools::{
        EDIT_TOOL_NAME, FIND_TOOL_NAME, GREP_TOOL_NAME, TERMINAL_TOOL_NAME, TREE_TOOL_NAME,
        WEB_SEARCH_TOOL_NAME, WRITE_TOOL_NAME,
    };

    /// Verifies that main chat agent gets built in tools.
    #[test]
    fn main_chat_agent_gets_built_in_tools() {
        let tools = main_chat_tool_storage(PathBuf::from("workspace"), PathBuf::from("trace")).unwrap();
        let runtime = RuntimeSelection {
            provider_type: OPENROUTER_PROVIDER_ID.to_owned(),
            provider_auth: Some(spectacular_config::ProviderAuthMode::ApiKey),
            provider: OPENROUTER_PROVIDER_ID.to_owned(),
            api_key: "sk-or-v1-test".to_owned(),
            model_key: "test-model".to_owned(),
            model: "test/model".to_owned(),
            reasoning: ReasoningLevel::Medium,
            context_window_tokens: None,
        };

        let main_agent = main_chat_agent(
            OpenRouterProvider::new(runtime.api_key.clone()),
            &runtime,
            Store::default(),
            tools,
        );

        assert_eq!(
            main_agent
                .tool_manifests()
                .into_iter()
                .map(|manifest| manifest.name)
                .collect::<Vec<_>>(),
            vec![
                EDIT_TOOL_NAME,
                FIND_TOOL_NAME,
                GREP_TOOL_NAME,
                TERMINAL_TOOL_NAME,
                TREE_TOOL_NAME,
                WEB_SEARCH_TOOL_NAME,
                WRITE_TOOL_NAME
            ]
        );
    }

    /// Verifies that context policy for runtime uses resolved window and reasoning reserve.
    #[test]
    fn context_policy_for_runtime_uses_resolved_window_and_reasoning_reserve() {
        let runtime = RuntimeSelection {
            provider_type: OPENROUTER_PROVIDER_ID.to_owned(),
            provider_auth: Some(spectacular_config::ProviderAuthMode::ApiKey),
            provider: OPENROUTER_PROVIDER_ID.to_owned(),
            api_key: "sk-test".to_owned(),
            model_key: "test-model".to_owned(),
            model: "test/model".to_owned(),
            reasoning: ReasoningLevel::Medium,
            context_window_tokens: Some(64_000),
        };

        let policy = context_policy_for_runtime(&runtime, runtime.context_window_tokens);

        assert_eq!(policy.model_context_window_tokens, Some(64_000));
        assert_eq!(policy.reasoning_reserve_tokens, 8_192);
        assert_eq!(policy.max_summary_passes_per_request, 4);
        assert!(policy.active_compaction_threshold().is_some());
    }

    /// Verifies that runtime context window falls back to provider metadata.
    #[test]
    fn runtime_context_window_falls_back_to_provider_metadata() {
        let runtime = RuntimeSelection {
            provider_type: OPENROUTER_PROVIDER_ID.to_owned(),
            provider_auth: Some(spectacular_config::ProviderAuthMode::ApiKey),
            provider: OPENROUTER_PROVIDER_ID.to_owned(),
            api_key: "sk-test".to_owned(),
            model_key: "test-model".to_owned(),
            model: "test/model".to_owned(),
            reasoning: ReasoningLevel::None,
            context_window_tokens: None,
        };
        let provider = OpenRouterProvider::new(String::new());

        let context_window_tokens = runtime_context_window_tokens(&provider, &runtime);
        let policy = context_policy_for_runtime(&runtime, context_window_tokens);

        assert_eq!(context_window_tokens, Some(32_768));
        assert_eq!(policy.model_context_window_tokens, Some(32_768));
        assert_eq!(policy.reasoning_reserve_tokens, 0);
    }
