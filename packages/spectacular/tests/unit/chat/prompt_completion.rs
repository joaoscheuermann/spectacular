    #[test]
    fn slash_query_shows_all_commands() {
        assert_eq!(suggestion_query("/", 1), Some(""));
    }

    #[test]
    fn command_prefix_query_is_extracted() {
        assert_eq!(suggestion_query("/his", 4), Some("his"));
    }

    #[test]
    fn suggestions_hide_after_arguments_start() {
        assert_eq!(suggestion_query("/history 2", 10), None);
    }

    #[test]
    fn suggestions_hide_after_newline() {
        assert_eq!(suggestion_query("/history\n2", 9), None);
    }

    #[test]
    fn suggestions_can_show_when_cursor_is_still_in_command_name() {
        assert_eq!(suggestion_query("/history 2", 4), Some("his"));
    }

    #[test]
    fn cursor_boundaries_handle_utf8() {
        let text = "a\u{00e9}/";
        assert_eq!(next_boundary(text, 0), 1);
        assert_eq!(next_boundary(text, 1), 3);
        assert_eq!(previous_boundary(text, 3), 1);
    }

    #[test]
    fn completion_adds_trailing_space() {
        let mut buffer = "/his".to_owned();
        let mut cursor = buffer.len();

        complete_suggestion(&mut buffer, &mut cursor, &test_suggestion("history"));

        assert_eq!(buffer, "/history ");
        assert_eq!(cursor, buffer.len());
    }

    #[test]
    fn completion_preserves_arguments() {
        let mut buffer = "/his 2".to_owned();
        let mut cursor = 4;

        complete_suggestion(&mut buffer, &mut cursor, &test_suggestion("history"));

        assert_eq!(buffer, "/history 2");
        assert_eq!(cursor, "/history ".len());
    }

    #[test]
    fn prompt_state_slash_suggests_all_commands() {
        let registry = test_registry();
        let completions = PromptCompletionCatalog::default();
        let mut state = PromptState::default();
        state.insert_char('/');

        let suggestions = state.suggestions(&registry, &completions);

        assert_eq!(suggestions.len(), 3);
    }

    #[test]
    fn prompt_state_filters_command_suggestions() {
        let registry = test_registry();
        let completions = PromptCompletionCatalog::default();
        let state = state_with("/his");

        let suggestions = state.suggestions(&registry, &completions);

        assert_eq!(suggestions[0].replacement, "history");
    }

    #[test]
    fn prompt_state_hides_suggestions_after_whitespace() {
        let registry = test_registry();
        let completions = PromptCompletionCatalog::default();
        let state = state_with("/history 2");

        assert!(state.suggestions(&registry, &completions).is_empty());
    }

    #[test]
    fn prompt_state_suggests_subcommands_after_command() {
        let registry = test_registry();
        let completions = test_completion_catalog();
        let state = state_with("/model ");

        let suggestions = state.suggestions(&registry, &completions);

        assert_eq!(
            suggestions
                .iter()
                .map(|suggestion| suggestion.replacement.as_str())
                .collect::<Vec<_>>(),
            vec!["add", "edit", "remove"]
        );
    }

    #[test]
    fn prompt_state_suggests_fields_after_subcommand() {
        let registry = test_registry();
        let completions = test_completion_catalog();
        let state = state_with("/model add pro");

        let suggestions = state.suggestions(&registry, &completions);

        assert_eq!(suggestions[0].replacement, "provider:");
        assert!(!suggestions[0].append_space);
    }

    #[test]
    fn prompt_state_suggests_dynamic_field_values() {
        let registry = test_registry();
        let completions = test_completion_catalog();
        let state = state_with("/model add provider:o");

        let suggestions = state.suggestions(&registry, &completions);

        assert_eq!(suggestions[0].replacement, "provider:openrouter");
    }

    #[test]
    fn prompt_state_suggests_provider_scoped_model_ids() {
        let registry = test_registry();
        let completions = test_completion_catalog();
        let state = state_with("/model add provider:work id:g");

        let suggestions = state.suggestions(&registry, &completions);

        assert_eq!(suggestions[0].replacement, "id:google/gemini");
        assert!(!suggestions
            .iter()
            .any(|suggestion| { suggestion.replacement == "id:openai/gpt-5.5" }));
    }

    #[test]
    fn enter_accepts_active_command_suggestion_before_submit() {
        let renderer = Renderer::default();
        let registry = Arc::new(test_registry());
        let completions = PromptCompletionCatalog::default();
        let mut editor = PromptEditor::new(&renderer, &registry, &completions);
        editor.state = state_with("/his");

        let action = editor
            .handle_event(Event::Key(KeyEvent::new(
                KeyCode::Enter,
                KeyModifiers::NONE,
            )))
            .unwrap();

        assert!(matches!(action, PromptAction::Continue));
        assert_eq!(editor.state.buffer, "/history ");
    }

    #[test]
    fn space_accepts_active_value_suggestion_in_command_mode() {
        let renderer = Renderer::default();
        let registry = Arc::new(test_registry());
        let completions = test_completion_catalog();
        let mut editor = PromptEditor::new(&renderer, &registry, &completions);
        editor.state = state_with("/model add provider:o");

        let action = editor
            .handle_event(Event::Key(KeyEvent::new(
                KeyCode::Char(' '),
                KeyModifiers::NONE,
            )))
            .unwrap();

        assert!(matches!(action, PromptAction::Continue));
        assert_eq!(editor.state.buffer, "/model add provider:openrouter id:");
        assert_eq!(editor.state.cursor, editor.state.buffer.len());
    }

    #[test]
    fn selecting_subcommand_inserts_next_required_field() {
        let renderer = Renderer::default();
        let registry = Arc::new(test_registry());
        let completions = test_completion_catalog();
        let mut editor = PromptEditor::new(&renderer, &registry, &completions);
        editor.state = state_with("/model ad");

        let action = editor
            .handle_event(Event::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)))
            .unwrap();

        assert!(matches!(action, PromptAction::Continue));
        assert_eq!(editor.state.buffer, "/model add provider:");
        assert_eq!(editor.state.cursor, editor.state.buffer.len());
    }

    #[test]
    fn enter_advances_to_next_missing_required_field() {
        let renderer = Renderer::default();
        let registry = Arc::new(test_registry());
        let completions = test_completion_catalog();
        let mut editor = PromptEditor::new(&renderer, &registry, &completions);
        editor.state = state_with("/model add provider:custom");

        let action = editor
            .handle_event(Event::Key(KeyEvent::new(
                KeyCode::Enter,
                KeyModifiers::NONE,
            )))
            .unwrap();

        assert!(matches!(action, PromptAction::Continue));
        assert_eq!(editor.state.buffer, "/model add provider:custom id:");
        assert_eq!(editor.state.cursor, editor.state.buffer.len());
    }

    #[test]
    fn enter_submits_complete_command_without_forcing_optional_field() {
        let renderer = Renderer::default();
        let registry = Arc::new(test_registry());
        let completions = test_completion_catalog();
        let mut editor = PromptEditor::new(&renderer, &registry, &completions);
        editor.state = state_with("/model add provider:work id:google/gemini reasoning:high ");

        let action = editor
            .handle_event(Event::Key(KeyEvent::new(
                KeyCode::Enter,
                KeyModifiers::NONE,
            )))
            .unwrap();

        assert!(matches!(action, PromptAction::Submit));
    }

    #[test]
    fn enter_blocks_invalid_static_choice_before_submit() {
        let renderer = Renderer::default();
        let registry = Arc::new(test_registry());
        let completions = test_completion_catalog();
        let mut editor = PromptEditor::new(&renderer, &registry, &completions);
        editor.state = state_with("/model add provider:work id:google/gemini reasoning:ultra");

        let action = editor
            .handle_event(Event::Key(KeyEvent::new(
                KeyCode::Enter,
                KeyModifiers::NONE,
            )))
            .unwrap();

        assert!(matches!(action, PromptAction::Continue));
        assert_eq!(
            editor.state.buffer,
            "/model add provider:work id:google/gemini reasoning:ultra"
        );
    }
