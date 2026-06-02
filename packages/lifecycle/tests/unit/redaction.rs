use lifecycle::redaction::{redact_failure_text, redact_repo_url};
use lifecycle::repo::RepoIdentity;
use lifecycle::terminal::safe_message;

#[test]
fn redact_repo_url_https_credentials_removes_userinfo() {
    let redacted = redact_repo_url("https://user:pass@host/path").expect("repo URL should parse");

    assert_eq!(redacted, "https://host/path");
    assert!(!redacted.contains("user"));
    assert!(!redacted.contains("pass"));
    assert!(!redacted.contains('@'));
}

#[test]
fn redact_repo_url_token_like_username_removes_userinfo() {
    let redacted = redact_repo_url("https://ghp_secret@github.com/org/repo.git")
        .expect("repo URL should parse");

    assert_eq!(redacted, "https://github.com/org/repo.git");
    assert!(!redacted.contains("ghp_secret"));
    assert!(!redacted.contains('@'));
}

#[test]
fn redact_repo_url_safe_query_and_fragment_preserves_them() {
    let redacted = redact_repo_url("https://user:pass@host/path?branch=main#readme")
        .expect("repo URL should parse");

    assert_eq!(redacted, "https://host/path?branch=main#readme");
    assert!(!redacted.contains("user"));
    assert!(!redacted.contains("pass"));
}

#[test]
fn redact_failure_text_api_key_like_token_replaces_secret() {
    let text = "provider failed with key sk-test_secret_1234567890abcdef while cloning";

    let redacted = redact_failure_text(text);

    assert!(redacted.contains("[REDACTED]"));
    assert!(!redacted.contains("sk-test_secret_1234567890abcdef"));
    assert!(redacted.contains("provider failed"));
}

#[test]
fn redact_failure_text_api_key_inside_provider_punctuation_replaces_secret() {
    let text =
        r#"provider returned {"error":"invalid key=sk-test_secret_1234567890abcdef; retry"}"#;

    let redacted = redact_failure_text(text);

    assert_eq!(
        redacted,
        r#"provider returned {"error":"invalid key=[REDACTED]; retry"}"#
    );
    assert!(!redacted.contains("sk-test_secret_1234567890abcdef"));
}

#[test]
fn repo_identity_from_raw_url_stores_redacted_display() {
    let identity = RepoIdentity::from_raw_url("https://user:pass@github.com/org/repo.git")
        .expect("repo identity should parse");

    assert_eq!(identity.as_str(), "https://github.com/org/repo.git");
    assert_eq!(identity.to_string(), "https://github.com/org/repo.git");
    assert!(!identity.as_str().contains("user"));
    assert!(!identity.as_str().contains("pass"));
    assert!(!identity.to_string().contains('@'));
}

#[test]
fn safe_message_line_breaks_and_terminal_controls_render_single_display_line() {
    let message = "provider\nfailed\rwith \u{1b}[31mcontrol\u{7} text";

    let safe: String = safe_message(message);

    assert!(safe.chars().all(|ch: char| !ch.is_control()));
    assert!(safe.contains("provider"));
    assert!(safe.contains("failed"));
    assert!(safe.contains("control"));
    assert!(safe.contains("text"));
}

#[test]
fn safe_message_api_key_like_token_redacts_before_display() {
    let safe: String = safe_message("provider failed with sk-test_secret_1234567890abcdef");

    assert_eq!(safe, "provider failed with [REDACTED]");
    assert!(!safe.contains("sk-test_secret_1234567890abcdef"));
}

#[test]
fn safe_message_credential_bearing_url_redacts_userinfo_before_display() {
    let safe: String = safe_message("cloning repo: https://user:pass@github.com/org/repo.git");

    assert_eq!(safe, "cloning repo: https://github.com/org/repo.git");
    assert!(!safe.contains("user:pass"));
    assert!(!safe.contains('@'));
}

#[test]
fn safe_message_malformed_credential_bearing_url_redacts_userinfo_before_display() {
    let raw_url = "https://user:pass@/path";

    let safe: String = safe_message("cloning repo: https://user:pass@/path");

    assert!(!safe.contains("user:pass"));
    assert!(!safe.contains(raw_url));
    assert!(safe.contains("cloning repo:"));
    assert!(safe.contains("/path") || safe.contains("[REDACTED]"));
}

#[test]
fn safe_message_ordinary_text_remains_readable() {
    let safe: String = safe_message("worker accepted: preparing repository");

    assert_eq!(safe, "worker accepted: preparing repository");
}
