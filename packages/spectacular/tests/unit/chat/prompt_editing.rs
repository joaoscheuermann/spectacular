    #[test]
    fn escape_cancels_active_suggestions_without_clearing_prompt() {
        let renderer = Renderer::default();
        let registry = Arc::new(test_registry());
        let completions = PromptCompletionCatalog::default();
        let mut editor = PromptEditor::new(&renderer, &registry, &completions);
        editor.state = state_with("/his");

        let action = editor
            .handle_event(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)))
            .unwrap();

        assert!(matches!(action, PromptAction::Continue));
        assert_eq!(editor.state.buffer, "/his");
        assert!(editor.state.suggestions(&registry, &completions).is_empty());

        let action = editor
            .handle_event(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)))
            .unwrap();

        assert!(matches!(action, PromptAction::Continue));
        assert_eq!(editor.state.buffer, "");
    }

    #[test]
    fn prompt_guidance_identifies_missing_required_field() {
        let model = test_model();
        let completions = test_completion_catalog(&model);

        let guidance = prompt_guidance("/test add provider:work ", 25, &completions);

        assert_eq!(
            guidance,
            vec![
                PromptGuidanceLine::Missing(vec!["id".to_owned(), "reasoning".to_owned()]),
                PromptGuidanceLine::Detail(
                    "id - model ID from the selected provider, required".to_owned()
                )
            ]
        );
    }

    #[test]
    fn prompt_guidance_lists_all_missing_required_fields() {
        let model = test_model();
        let completions = test_completion_catalog(&model);

        let guidance = prompt_guidance(
            "/test add provider:",
            "/test add provider:".len(),
            &completions,
        );

        assert_eq!(
            guidance,
            vec![
                PromptGuidanceLine::Missing(vec![
                    "provider".to_owned(),
                    "id".to_owned(),
                    "reasoning".to_owned()
                ]),
                PromptGuidanceLine::Detail(
                    "provider - configured provider name, required".to_owned()
                )
            ]
        );
    }

    #[test]
    fn missing_guidance_renders_dim_orange_with_bold_label() {
        let rendered = render_guidance_line(&PromptGuidanceLine::Missing(vec![
            "provider".to_owned(),
            "id".to_owned(),
            "reasoning".to_owned(),
        ]));

        assert!(rendered.contains("\x1b[2m"));
        assert!(rendered.contains("\x1b[1m"));
        assert!(rendered.contains("\x1b[38;2;251;191;36m"));
        assert!(rendered.contains("missing"));
        assert!(rendered.contains("provider, id, reasoning."));
    }

    #[test]
    fn value_suggestions_include_more_count_for_long_lists() {
        let registry = test_registry();
        let model = test_model();
        let completions = test_completion_catalog(&model);
        let state = state_with("/test add provider:busy id:");

        let suggestions = state.suggestions(&registry, &completions);

        assert_eq!(suggestions.len(), MAX_SUGGESTIONS + 1);
        assert_eq!(
            suggestions
                .last()
                .map(|suggestion| suggestion.label.as_str()),
            Some("[more 2 items...]")
        );
        assert_eq!(
            suggestions.last().map(|suggestion| suggestion.kind),
            Some(PromptSuggestionKind::Info)
        );
    }

    #[test]
    fn prompt_state_selection_clamps() {
        let mut state = state_with("/");

        state.select_next(2);
        state.select_next(2);
        state.select_next(2);
        assert_eq!(state.selected, 1);
        state.select_previous();
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn prompt_state_completion_preserves_arguments() {
        let mut state = state_with("/his 2");
        state.cursor = 4;

        state.complete_suggestion(&test_suggestion("history"));

        assert_eq!(state.buffer, "/history 2");
        assert_eq!(state.cursor, "/history ".len());
    }

    #[test]
    fn prompt_state_inserts_multiline_text() {
        let mut state = state_with("hello");

        state.insert_str("\nworld");

        assert_eq!(state.buffer, "hello\nworld");
        assert_eq!(state.cursor, state.buffer.len());
    }

    #[test]
    fn prompt_state_replaces_selection_on_insert() {
        let mut state = state_with("hello");
        state.cursor = 1;
        state.move_right(true);
        state.move_right(true);

        state.insert_char('X');

        assert_eq!(state.buffer, "hXlo");
        assert_eq!(state.cursor, 2);
        assert_eq!(state.selection_range(), None);
    }

    #[test]
    fn prompt_state_deletes_selection_on_backspace() {
        let mut state = state_with("hello");
        state.cursor = 1;
        state.move_right(true);
        state.move_right(true);

        state.backspace();

        assert_eq!(state.buffer, "hlo");
        assert_eq!(state.cursor, 1);
    }

    #[test]
    fn prompt_state_select_all_replaces_buffer() {
        let mut state = state_with("hello");

        state.select_all();
        state.insert_str("bye");

        assert_eq!(state.buffer, "bye");
        assert_eq!(state.cursor, 3);
    }

    #[test]
    fn prompt_state_home_and_end_use_current_line() {
        let mut state = state_with("one\ntwo\nthree");
        state.cursor = "one\ntwo".len();

        state.move_line_start(false);
        assert_eq!(state.cursor, "one\n".len());

        state.move_line_end(false);
        assert_eq!(state.cursor, "one\ntwo".len());
    }

    #[test]
    fn prompt_state_ctrl_home_and_end_use_whole_buffer() {
        let mut state = state_with("one\ntwo\nthree");
        state.cursor = "one\ntwo".len();

        state.move_start(false);
        assert_eq!(state.cursor, 0);

        state.move_end(false);
        assert_eq!(state.cursor, state.buffer.len());
    }

    #[test]
    fn prompt_state_vertical_movement_uses_visual_rows() {
        let mut state = state_with("abcd");

        state.move_visual_up(false, 2);
        assert_eq!(state.cursor, 2);

        state.move_visual_down(false, 2);
        assert_eq!(state.cursor, 4);
    }

    #[test]
    fn prompt_state_vertical_selection_extends_anchor() {
        let mut state = state_with("abcd");

        state.move_visual_up(true, 2);

        assert_eq!(state.selection_range(), Some(2..4));
    }

    #[test]
    fn prompt_state_deletes_previous_word() {
        let mut state = state_with("hello world");

        state.delete_previous_word();

        assert_eq!(state.buffer, "hello ");
        assert_eq!(state.cursor, "hello ".len());
    }

    #[test]
    fn prompt_state_kill_and_yank_round_trip_current_line() {
        let mut state = state_with("one\ntwo");

        state.kill_to_line_start();
        assert_eq!(state.buffer, "one\n");
        assert_eq!(state.kill_buffer, "two");

        state.yank();
        assert_eq!(state.buffer, "one\ntwo");
    }

    #[test]
    fn visual_rows_wrap_and_preserve_empty_lines() {
        let state = state_with("ab\n\ncde\n");

        assert_eq!(
            state.visual_rows(2),
            vec![
                VisualRow { start: 0, end: 2 },
                VisualRow { start: 3, end: 3 },
                VisualRow { start: 4, end: 6 },
                VisualRow { start: 6, end: 7 },
                VisualRow { start: 8, end: 8 },
            ]
        );
    }

    #[test]
    fn paste_normalization_uses_lf() {
        assert_eq!(normalize_paste("a\r\nb\rc"), "a\nb\nc");
    }

    #[test]
    fn bracketed_paste_normalizes_crlf_without_submitting() {
        let renderer = Renderer::default();
        let registry = Arc::new(test_registry());
        let completions = PromptCompletionCatalog::default();
        let mut editor = PromptEditor::new(&renderer, &registry, &completions);

        let action = editor
            .handle_event(Event::Paste("a\r\nb".to_owned()))
            .unwrap();

        assert!(matches!(action, PromptAction::Continue));
        assert_eq!(editor.state.buffer, "a\nb");
    }

    #[test]
    fn unbracketed_crlf_paste_keeps_line_breaks_without_submit() {
        let renderer = Renderer::default();
        let registry = Arc::new(test_registry());
        let completions = PromptCompletionCatalog::default();
        let mut editor = PromptEditor::new(&renderer, &registry, &completions);
        let pasted =
            "error: variants `PowerShell` and `Cmd` are never constructed\r\n\r\nError: build failed";
        let mut now = Instant::now();

        for key in unbracketed_paste_keys(pasted) {
            let action = editor.handle_key_with_time(key, now).unwrap();
            assert!(matches!(
                action,
                PromptAction::Noop | PromptAction::Continue
            ));
            now += std::time::Duration::from_millis(1);
        }

        let flush_at = now
            + PasteBurst::recommended_active_flush_delay()
            + std::time::Duration::from_millis(1);
        assert!(editor.flush_paste_burst_if_due(flush_at));
        assert_eq!(editor.state.buffer, normalize_paste(pasted));
    }

    #[test]
    fn plain_enter_submits_after_pending_character_flushes() {
        let renderer = Renderer::default();
        let registry = Arc::new(test_registry());
        let completions = PromptCompletionCatalog::default();
        let mut editor = PromptEditor::new(&renderer, &registry, &completions);
        let now = Instant::now();

        let action = editor
            .handle_key_with_time(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE), now)
            .unwrap();
        assert!(matches!(action, PromptAction::Noop));
        assert_eq!(editor.state.buffer, "");

        let flush_at = now + PasteBurst::recommended_flush_delay();
        assert!(editor.flush_paste_burst_if_due(flush_at));
        assert_eq!(editor.state.buffer, "h");

        let action = editor
            .handle_key_with_time(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                flush_at + std::time::Duration::from_millis(1),
            )
            .unwrap();
        assert!(matches!(action, PromptAction::Submit));
    }

    #[test]
    fn unbracketed_paste_does_not_redraw_until_flush() {
        let renderer = Renderer::default();
        let registry = Arc::new(test_registry());
        let completions = PromptCompletionCatalog::default();
        let mut editor = PromptEditor::new(&renderer, &registry, &completions);
        let now = Instant::now();

        let first = editor
            .handle_key_with_time(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE), now)
            .unwrap();
        let second = editor
            .handle_key_with_time(
                KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
                now + std::time::Duration::from_millis(1),
            )
            .unwrap();
        let newline = editor
            .handle_key_with_time(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                now + std::time::Duration::from_millis(2),
            )
            .unwrap();

        assert!(matches!(first, PromptAction::Noop));
        assert!(matches!(second, PromptAction::Noop));
        assert!(matches!(newline, PromptAction::Noop));
        assert_eq!(editor.state.buffer, "");

        let flush_at = now
            + std::time::Duration::from_millis(2)
            + PasteBurst::recommended_active_flush_delay()
            + std::time::Duration::from_millis(1);
        assert!(editor.flush_paste_burst_if_due(flush_at));
        assert_eq!(editor.state.buffer, "ab\n");
    }
