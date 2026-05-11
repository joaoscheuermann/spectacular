    use super::*;
    use crate::chat::title::title_generation_agent;
    use spectacular_config::ReasoningLevel;
    use spectacular_llms::{MessageDelta, OpenRouterProvider, ReasoningDelta, OPENROUTER_PROVIDER_ID};
    use spectacular_tools::{
        EDIT_TOOL_NAME, FIND_TOOL_NAME, GREP_TOOL_NAME, TERMINAL_TOOL_NAME, TREE_TOOL_NAME,
        WEB_SEARCH_TOOL_NAME, WRITE_TOOL_NAME,
    };

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

    #[test]
    fn retry_user_prompt_gate_skips_first_user_prompt() {
        let mut skip_retry_user = true;

        let skipped =
            should_skip_retry_user_prompt(&mut skip_retry_user, &AgentEvent::user_prompt("again"));

        assert_eq!((skipped, skip_retry_user), (true, false));
    }

    #[test]
    fn title_task_triggers_after_nonblank_assistant_text() {
        assert!(should_spawn_title_task(false, "answer"));
    }

    #[test]
    fn assistant_response_render_state_hides_blank_response() {
        let mut state = AssistantResponseRenderState::default();

        assert!(state.delta("").is_none());
        assert!(state.delta(" \n\t").is_none());
        assert!(!state.close_visible_response());
    }

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

    #[test]
    fn cancelled_agent_event_is_terminal() {
        assert!(is_terminal_agent_event(&AgentEvent::cancelled("stopped")));
    }

    #[test]
    fn assistant_events_render_before_append() {
        let event = AgentEvent::MessageDelta(MessageDelta::assistant("hello"));

        assert!(!should_append_without_render(&event));
    }

    #[test]
    fn reasoning_events_render_before_append() {
        let event = AgentEvent::ReasoningDelta(ReasoningDelta {
            content: "thinking".to_owned(),
            metadata: None,
        });

        assert!(!should_append_without_render(&event));
    }
