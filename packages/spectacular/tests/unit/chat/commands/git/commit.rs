    use super::*;
    use crate::chat::commands::{test_support::NoopRunner, ChatCommandControl, ChatCommandResult};
    use crate::chat::model::ChatModel;
    use crate::chat::renderer::Renderer;
    use crate::chat::session::{ChatEvent, SessionManager};
    use crate::chat::RuntimeSelection;
    use spectacular_agent::ToolStorage;
    use spectacular_config::ReasoningLevel;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn sanitize_removes_markdown_fences() {
        let input = "```\nfix(auth): resolve token issue\n```";
        let result = sanitize_commit_message(input);
        assert_eq!(result, "fix(auth): resolve token issue");
    }

    #[test]
    fn sanitize_removes_conversational_prefix() {
        let input = "Here is the commit message:\n\nfeat: add user authentication";
        let result = sanitize_commit_message(input);
        assert_eq!(result, "feat: add user authentication");
    }

    #[test]
    fn sanitize_removes_commit_message_label() {
        let input = "Commit message:\nfix: handle null pointer";
        let result = sanitize_commit_message(input);
        assert_eq!(result, "fix: handle null pointer");
    }

    #[test]
    fn sanitize_trims_and_removes_quotes() {
        let input = "  \"feat: add new feature\"  ";
        let result = sanitize_commit_message(input);
        assert_eq!(result, "feat: add new feature");
    }

    #[test]
    fn truncate_diff_keeps_small_diffs_intact() {
        let diff = "small diff content";
        let (result, truncated) = truncate_diff_if_needed(diff);
        assert!(!truncated);
        assert_eq!(result, diff);
    }

    #[test]
    fn truncate_diff_truncates_large_diffs() {
        let diff = "x".repeat(MAX_DIFF_CHARS + 1000);
        let (result, truncated) = truncate_diff_if_needed(&diff);
        assert!(truncated);
        assert!(result.contains("[diff truncated"));
        assert!(result.len() < diff.len());
    }


    #[test]
    fn command_lifecycle_bounds_are_explicit() {
        assert_eq!(MAX_COMMAND_TEXT_CHARS, 80);
        assert_eq!(MAX_COMMAND_DELTA_EVENTS, 32);
        assert_eq!(MAX_COMMAND_DELTA_BYTES, 4_096);
        assert_eq!(MAX_COMMAND_DELTA_CONTENT_CHARS, 240);
        assert_eq!(MAX_COMMAND_SUMMARY_CHARS, 240);
        assert_eq!(TEXT_TRUNCATION_MARKER, "... [truncated]");
        assert_eq!(
            COMMAND_DELTA_TRUNCATED_NOTICE,
            "command output truncated: persistence limit reached"
        );
    }

    #[test]
    fn bounded_text_truncates_by_characters() {
        assert_eq!(bounded_text("abcdef", 3), "...");
        assert_eq!(
            bounded_text("abcdefghijklmnopqrstuvwxyz", 18),
            "abc... [truncated]"
        );
        assert_eq!(bounded_text("abc", 3), "abc");
    }


    #[tokio::test]
    async fn git_commit_invalid_args_persist_lifecycle_records() {
        let mut model = test_model();
        let renderer = Renderer::default();
        let tools = ToolStorage::default();
        let runner = NoopRunner;
        let mut control = ChatCommandControl::default();
        let context = ChatCommandContext::new(&mut model, &renderer, &tools, &runner, &mut control);

        let result = execute(context, vec!["extra".to_owned()]).await;

        assert!(matches!(result, ChatCommandResult::Error(_)));
        let records = model.records().unwrap();
        let command_events = records
            .iter()
            .filter_map(|record| record.event())
            .filter(|event| {
                matches!(
                    event,
                    ChatEvent::CommandStart { .. }
                        | ChatEvent::CommandDelta { .. }
                        | ChatEvent::CommandFinished { .. }
                )
            })
            .collect::<Vec<_>>();

        assert!(matches!(command_events[0], ChatEvent::CommandStart { .. }));
        assert!(matches!(
            command_events[1],
            ChatEvent::CommandFinished { status, .. } if status == "failed"
        ));
    }

    #[tokio::test]
    async fn command_lifecycle_persists_commit_output_before_success() {
        let mut model = test_model();
        let renderer = Renderer::default();
        let tools = ToolStorage::default();
        let runner = NoopRunner;
        let mut control = ChatCommandControl::default();

        {
            let context =
                ChatCommandContext::new(&mut model, &renderer, &tools, &runner, &mut control);
            let mut lifecycle = CommitLifecycle::new(&context);
            lifecycle.start().unwrap();
            lifecycle.delta("committing changes").unwrap();
            lifecycle
                .delta("[feat/test abc1234] feat: persist commit output")
                .unwrap();
            lifecycle
                .finish(CommandStatus::Success, "changes committed successfully")
                .unwrap();
        }

        let records = model.records().unwrap();
        let command_events = records
            .iter()
            .filter_map(|record| record.event())
            .filter_map(|event| match event {
                ChatEvent::CommandDelta { content, .. } => Some(content.as_str()),
                ChatEvent::CommandFinished { summary, .. } => Some(summary.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            command_events,
            vec![
                "committing changes",
                "[feat/test abc1234] feat: persist commit output",
                "changes committed successfully",
            ]
        );
    }

    #[tokio::test]
    async fn command_lifecycle_persists_cancelled_status() {
        let mut model = test_model();
        let renderer = Renderer::default();
        let tools = ToolStorage::default();
        let runner = NoopRunner;
        let mut control = ChatCommandControl::default();

        {
            let context =
                ChatCommandContext::new(&mut model, &renderer, &tools, &runner, &mut control);
            let lifecycle = CommitLifecycle::new(&context);
            lifecycle.start().unwrap();
            lifecycle
                .finish(CommandStatus::Cancelled, "commit cancelled")
                .unwrap();
        }

        let records = model.records().unwrap();
        let finished = records
            .iter()
            .filter_map(|record| record.event())
            .find_map(|event| match event {
                ChatEvent::CommandFinished { status, summary, .. } => Some((status, summary)),
                _ => None,
            })
            .unwrap();

        assert_eq!(finished.0, "cancelled");
        assert_eq!(finished.1, "commit cancelled");
    }

    #[tokio::test]
    async fn command_lifecycle_records_truncation_notice_before_event_limit() {
        let mut model = test_model();
        let renderer = Renderer::default();
        let tools = ToolStorage::default();
        let runner = NoopRunner;
        let mut control = ChatCommandControl::default();

        {
            let context =
                ChatCommandContext::new(&mut model, &renderer, &tools, &runner, &mut control);
            let mut lifecycle = CommitLifecycle::new(&context);
            lifecycle.start().unwrap();
            for index in 0..(MAX_COMMAND_DELTA_EVENTS + 4) {
                lifecycle.delta(&format!("phase {index}")).unwrap();
            }
        }

        let records = model.records().unwrap();
        let deltas = records
            .iter()
            .filter_map(|record| record.event())
            .filter_map(|event| match event {
                ChatEvent::CommandDelta { content, .. } => Some(content.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(deltas.len(), MAX_COMMAND_DELTA_EVENTS);
        assert_eq!(deltas.last(), Some(&COMMAND_DELTA_TRUNCATED_NOTICE));
    }

    #[tokio::test]
    async fn command_lifecycle_records_truncation_notice_before_byte_limit() {
        let mut model = test_model();
        let renderer = Renderer::default();
        let tools = ToolStorage::default();
        let runner = NoopRunner;
        let mut control = ChatCommandControl::default();

        {
            let context =
                ChatCommandContext::new(&mut model, &renderer, &tools, &runner, &mut control);
            let mut lifecycle = CommitLifecycle::new(&context);
            lifecycle.start().unwrap();
            let content = "x".repeat(MAX_COMMAND_DELTA_CONTENT_CHARS);
            for _ in 0..MAX_COMMAND_DELTA_EVENTS {
                lifecycle.delta(&content).unwrap();
            }
        }

        let records = model.records().unwrap();
        let deltas = records
            .iter()
            .filter_map(|record| record.event())
            .filter_map(|event| match event {
                ChatEvent::CommandDelta { content, .. } => Some(content.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let total_bytes = deltas.iter().map(|content| content.len()).sum::<usize>();

        assert!(total_bytes <= MAX_COMMAND_DELTA_BYTES);
        assert_eq!(deltas.last(), Some(&COMMAND_DELTA_TRUNCATED_NOTICE));
    }

    #[test]
    fn prompt_includes_diff_content() {
        let diff = "+ let x = 1;";
        let prompt = build_commit_prompt(diff);
        assert!(prompt.contains(diff));
        assert!(prompt.contains("conventional commit message"));
    }

    /// Builds a chat model configured for command tests.
    fn test_model() -> ChatModel {
        let session = SessionManager::new_in(temp_session_dir("git-commit")).unwrap();
        let mut model = ChatModel::new(
            session,
            RuntimeSelection {
                provider_type: "openrouter".to_owned(),
                provider_auth: Some(spectacular_config::ProviderAuthMode::ApiKey),
                provider: "openrouter".to_owned(),
                api_key: "sk-or-v1-test".to_owned(),
                model_key: "test-model".to_owned(),
                model: "test/model".to_owned(),
                reasoning: ReasoningLevel::Medium,
                context_window_tokens: None,
            },
        );
        model.start_new_session().unwrap();
        model
    }

    /// Builds a temporary session directory path for a named test case.
    fn temp_session_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        std::env::temp_dir().join(format!("spectacular-git-commit-command-{name}-{suffix}"))
    }
