use crate::chat::{ChatError, RuntimeSelection};
use spectacular_llms::{LlmDebugLogger, OpenRouterProvider, OPENROUTER_PROVIDER_ID};

pub fn provider_for_runtime(
    runtime: &RuntimeSelection,
    debug_logger: LlmDebugLogger,
) -> Result<OpenRouterProvider, ChatError> {
    if runtime.provider == OPENROUTER_PROVIDER_ID {
        return Ok(OpenRouterProvider::with_debug_logger(
            runtime.api_key.clone(),
            debug_logger,
        ));
    }

    Err(ChatError::Session(format!(
        "provider `{}` is not supported by chat",
        runtime.provider
    )))
}

pub fn provider_for_parts(
    provider: &str,
    api_key: String,
    debug_logger: LlmDebugLogger,
) -> Result<OpenRouterProvider, ChatError> {
    if provider == OPENROUTER_PROVIDER_ID {
        return Ok(OpenRouterProvider::with_debug_logger(api_key, debug_logger));
    }

    Err(ChatError::Session(format!(
        "provider `{provider}` is not supported by chat"
    )))
}
