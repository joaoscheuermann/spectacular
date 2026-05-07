    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tokio::fs;

    async fn temp_git_repo(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("spectacular-git-helpers-{}-{}", name, suffix));
        fs::create_dir_all(&dir).await.unwrap();

        let shell = ShellSpec::detect();
        for cmd in &[
            "git init",
            "git config user.email \"test@test.com\"",
            "git config user.name \"Test\"",
        ] {
            let mut c = shell.to_command(cmd);
            c.current_dir(&dir);
            c.status().await.unwrap();
        }

        dir
    }

    #[tokio::test]
    async fn staged_change_detection_reports_false_then_true() {
        let dir = temp_git_repo("staged-detection").await;
        let original = std::env::current_dir().unwrap();
        let _ = std::env::set_current_dir(&dir);

        let has_staged = has_staged_changes().await.unwrap();
        assert!(!has_staged);

        fs::write(dir.join("file.txt"), "staged content\n")
            .await
            .unwrap();
        let shell = ShellSpec::detect();
        let mut add = shell.to_command("git add file.txt");
        add.current_dir(&dir);
        assert!(add.status().await.unwrap().success());

        let has_staged = has_staged_changes().await.unwrap();

        let _ = std::env::set_current_dir(&original);
        let _ = fs::remove_dir_all(&dir).await;

        assert!(has_staged);
    }

    #[test]
    fn shell_escape_wraps_args_with_parentheses() {
        let arg = "feat(chat): add new feature";
        let escaped = shell_escape(arg);
        assert_eq!(escaped, "\"feat(chat): add new feature\"");
    }

    #[test]
    fn shell_escape_wraps_multiline_args() {
        let arg = "fix: bug\n\nThis fixes a bug.";
        let escaped = shell_escape(arg);
        assert!(escaped.starts_with('"') && escaped.ends_with('"'));
    }

    #[test]
    fn shell_escape_escapes_internal_quotes() {
        let arg = "fix: \"something\" broke";
        let escaped = shell_escape(arg);
        assert_eq!(escaped, "\"fix: \\\"something\\\" broke\"");
    }

    #[test]
    fn shell_escape_plain_arg_unchanged() {
        let arg = "simple-arg";
        let escaped = shell_escape(arg);
        assert_eq!(escaped, "simple-arg");
    }
