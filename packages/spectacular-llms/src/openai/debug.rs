use crate::debug as provider_debug;
use crate::{LlmDebugLogger, ProviderError};

const TARGET: &str = "openai";

/// Writes an OpenAI debug event without failing the provider call.
pub(crate) fn log_event(logger: &LlmDebugLogger, event: &str, fields: serde_json::Value) {
    provider_debug::log_event(TARGET, logger, event, fields);
}

/// Writes a raw JSON OpenAI debug event without failing the provider call.
pub(crate) fn log_raw_json(logger: &LlmDebugLogger, event: &str, raw_json: serde_json::Value) {
    provider_debug::log_raw_json(TARGET, logger, event, raw_json);
}

/// Writes a raw text OpenAI debug event without failing the provider call.
pub(crate) fn log_raw_text(logger: &LlmDebugLogger, event: &str, raw_text: &str) {
    provider_debug::log_raw_text(TARGET, logger, event, raw_text);
}

/// Writes an OpenAI error debug event without failing the provider call.
pub(crate) fn log_error(logger: &LlmDebugLogger, event: &str, error: &ProviderError) {
    provider_debug::log_error(TARGET, logger, event, error);
}
