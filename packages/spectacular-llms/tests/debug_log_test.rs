use serde_json::json;
use spectacular_llms::LlmDebugLogger;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn debug_logger_creates_and_truncates_file() {
    let path = temp_log_path("truncate");
    std::fs::write(&path, "stale content").unwrap();

    let logger = LlmDebugLogger::create_at_path(&path).unwrap();

    assert_eq!(logger.path(), Some(path.as_path()));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "");
}

#[test]
fn debug_logger_writes_valid_jsonl_records() {
    let path = temp_log_path("jsonl");
    let logger = LlmDebugLogger::create_at_path(&path).unwrap();

    logger
        .write_event("openrouter", "response_status", json!({ "status": 200 }))
        .unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    let record: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
    assert_eq!(record["target"], "openrouter");
    assert_eq!(record["event"], "response_status");
    assert_eq!(record["status"], 200);
    assert!(record["ts_ms"].is_u64());
}

#[test]
fn debug_logger_records_are_readable_before_drop() {
    let path = temp_log_path("live");
    let logger = LlmDebugLogger::create_at_path(&path).unwrap();

    logger
        .write_event("openrouter", "stream_started", json!({}))
        .unwrap();

    assert!(std::fs::read_to_string(&path)
        .unwrap()
        .contains("stream_started"));
}

#[test]
fn debug_logger_preserves_raw_provider_payloads() {
    let path = temp_log_path("raw");
    let logger = LlmDebugLogger::create_at_path(&path).unwrap();
    let payload = json!({
        "choices": [
            {
                "delta": {
                    "content": "hello"
                }
            }
        ]
    });

    logger
        .write_raw_json("openrouter", "sse_payload", payload.clone())
        .unwrap();
    logger
        .write_raw_text("openrouter", "sse_payload_text", r#"{"done":true}"#)
        .unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("\"content\":\"hello\""));
    assert!(content.contains(r#""raw_text":"{\"done\":true}""#));
}

#[test]
fn debug_logger_metadata_does_not_add_api_keys() {
    let path = temp_log_path("redacted");
    let logger = LlmDebugLogger::create_at_path(&path).unwrap();
    let secret = "sk-or-v1-secret";

    logger
        .write_event(
            "openrouter",
            "api_key_validation_response_status",
            json!({ "status": 200 }),
        )
        .unwrap();

    assert!(!std::fs::read_to_string(&path).unwrap().contains(secret));
}

#[test]
fn disabled_debug_logger_is_noop() {
    let logger = LlmDebugLogger::disabled();

    logger
        .write_raw_text("openrouter", "ignored", "payload")
        .unwrap();

    assert_eq!(logger.path(), None);
}

fn temp_log_path(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    std::env::temp_dir().join(format!("spectacular-llms-debug-log-{name}-{suffix}.log"))
}
