    use super::*;
    use crate::chat::title::title_generation_agent;
    use spectacular_config::ReasoningLevel;
    use spectacular_llms::{
        MessageDelta, OpenRouterProvider, ReasoningDelta, OPENROUTER_PROVIDER_ID,
    };
    use spectacular_tools::{
        EDIT_TOOL_NAME, FIND_TOOL_NAME, GREP_TOOL_NAME, TERMINAL_TOOL_NAME, TREE_TOOL_NAME,
        WEB_SEARCH_TOOL_NAME, WRITE_TOOL_NAME,
    };

    /// Verifies that main chat agent gets built in tools and title agent stays text only.
    #[test]
    fn main_chat_agent_gets_built_in_tools_and_title_agent_stays_text_only() {
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
        let title_agent = title_generation_agent(
            OpenRouterProvider::new(runtime.api_key),
            "title/model".to_owned(),
            "Generate a title.".to_owned(),
            Store::default(),
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
        assert!(title_agent.tool_manifests().is_empty());
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

    /// Verifies that retry user prompt gate skips first user prompt.
    #[test]
    fn retry_user_prompt_gate_skips_first_user_prompt() {
        let mut skip_retry_user = true;

        let skipped =
            should_skip_retry_user_prompt(&mut skip_retry_user, &AgentEvent::user_prompt("again"));

        assert_eq!((skipped, skip_retry_user), (true, false));
    }

    /// Verifies that title task triggers after nonblank assistant text.
    #[test]
    fn title_task_triggers_after_nonblank_assistant_text() {
        assert!(should_spawn_title_task(false, "answer"));
    }

    /// Verifies that assistant response render state hides blank response.
    #[test]
    fn assistant_response_render_state_hides_blank_response() {
        let mut state = AssistantResponseRenderState::default();

        assert!(state.delta("").is_none());
        assert!(state.delta(" \n\t").is_none());
        assert!(!state.close_visible_response());
    }

    /// Verifies that assistant response render state starts when aggregate becomes nonblank.
    #[test]
    fn assistant_response_render_state_starts_when_aggregate_becomes_nonblank() {
        let mut state = AssistantResponseRenderState::default();

        assert!(state.delta("\n").is_none());
        let render = state
            .delta("answer")
            .expect("nonblank aggregate should become visible");

        assert!(render.started);
        assert_eq!(render.content, "\nanswer");
        assert!(state.close_visible_response());
    }

    /// Verifies that reasoning response render state starts once and then appends.
    #[test]
    fn reasoning_response_render_state_starts_once_and_then_appends() {
        let mut state = ReasoningResponseRenderState::default();

        assert!(state.delta("\n").is_none());
        let first = state
            .delta("thinking")
            .expect("nonblank reasoning should become visible");
        let second = state
            .delta(" more")
            .expect("visible reasoning should continue streaming");

        assert!(first.started);
        assert_eq!(first.content, "\nthinking");
        assert!(!second.started);
        assert_eq!(second.content, " more");
        assert!(state.close_visible_response());
    }

    /// Verifies that cancelled agent event is terminal.
    #[test]
    fn cancelled_agent_event_is_terminal() {
        assert!(is_terminal_agent_event(&AgentEvent::cancelled("stopped")));
    }

    /// Verifies that assistant events render before append.
    #[test]
    fn assistant_events_render_before_append() {
        let event = AgentEvent::MessageDelta(MessageDelta::assistant("hello"));

        assert!(!should_append_without_render(&event));
    }

    /// Verifies that reasoning events render before append.
    #[test]
    fn reasoning_events_render_before_append() {
        let event = AgentEvent::ReasoningDelta(ReasoningDelta {
            content: "thinking".to_owned(),
            metadata: None,
        });

        assert!(!should_append_without_render(&event));
    }
