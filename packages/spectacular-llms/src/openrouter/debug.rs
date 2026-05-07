use crate::{LlmDebugLogger, ProviderError, ProviderFinished};
use serde_json::json;

const TARGET: &str = "openrouter";

pub(crate) fn log_event(logger: &LlmDebugLogger, event: &str, fields: serde_json::Value) {
    let _ = logger.write_event(TARGET, event, fields);
}

pub(crate) fn log_raw_json(logger: &LlmDebugLogger, event: &str, raw_json: serde_json::Value) {
    let _ = logger.write_raw_json(TARGET, event, raw_json);
}

pub(crate) fn log_raw_text(logger: &LlmDebugLogger, event: &str, raw_text: &str) {
    let _ = logger.write_raw_text(TARGET, event, raw_text);
}

pub(crate) fn log_error(logger: &LlmDebugLogger, event: &str, error: &ProviderError) {
    log_event(
        logger,
        event,
        json!({
            "error_kind": format!("{error:?}"),
            "message": error.to_string(),
        }),
    );
}

pub(crate) fn log_finish(logger: &LlmDebugLogger, event: &str, finished: &ProviderFinished) {
    let usage = finished.usage.map(|usage| {
        json!({
            "input_tokens": usage.input_tokens,
            "output_tokens": usage.output_tokens,
            "total_tokens": usage.total_tokens,
        })
    });

    log_event(
        logger,
        event,
        json!({
            "finish_reason": format!("{:?}", finished.finish_reason),
            "tool_call_count": finished.tool_calls.len(),
            "usage": usage,
            "reasoning": finished.reasoning.as_ref().map(|reasoning| {
                json!({
                    "effort": reasoning.effort,
                    "summary": reasoning.summary,
                })
            }),
        }),
    );
}
