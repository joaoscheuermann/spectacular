use crate::{LlmDebugLogger, ProviderError, ProviderFinished};
use serde_json::json;

/// Writes a provider-scoped debug event without failing the provider call.
pub(crate) fn log_event(
    target: &str,
    logger: &LlmDebugLogger,
    event: &str,
    fields: serde_json::Value,
) {
    let _ = logger.write_event(target, event, fields);
}

/// Writes a provider-scoped raw JSON debug event without failing the provider call.
pub(crate) fn log_raw_json(
    target: &str,
    logger: &LlmDebugLogger,
    event: &str,
    raw_json: serde_json::Value,
) {
    let _ = logger.write_raw_json(target, event, raw_json);
}

/// Writes a provider-scoped raw text debug event without failing the provider call.
pub(crate) fn log_raw_text(target: &str, logger: &LlmDebugLogger, event: &str, raw_text: &str) {
    let _ = logger.write_raw_text(target, event, raw_text);
}

/// Writes a provider-scoped error debug event without failing the provider call.
pub(crate) fn log_error(target: &str, logger: &LlmDebugLogger, event: &str, error: &ProviderError) {
    log_event(
        target,
        logger,
        event,
        json!({
            "error_kind": format!("{error:?}"),
            "message": error.to_string(),
        }),
    );
}

/// Writes a provider-scoped finish debug event without failing the provider call.
pub(crate) fn log_finish(
    target: &str,
    logger: &LlmDebugLogger,
    event: &str,
    finished: &ProviderFinished,
) {
    let usage = finished.usage.map(|usage| {
        json!({
            "input_tokens": usage.input_tokens,
            "output_tokens": usage.output_tokens,
            "total_tokens": usage.total_tokens,
        })
    });

    log_event(
        target,
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
