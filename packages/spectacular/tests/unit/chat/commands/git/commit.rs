    use super::*;

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
    fn truncated_diff_notice_is_ascii() {
        assert_eq!(
            TRUNCATED_DIFF_NOTICE,
            "warning: diff is large and has been truncated for the commit message agent"
        );
        assert!(TRUNCATED_DIFF_NOTICE.is_ascii());
    }

    #[test]
    fn prompt_includes_diff_content() {
        let diff = "+ let x = 1;";
        let prompt = build_commit_prompt(diff);
        assert!(prompt.contains(diff));
        assert!(prompt.contains("conventional commit message"));
    }
