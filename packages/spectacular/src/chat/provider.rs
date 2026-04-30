use crate::chat::{ChatError, RuntimeSelection};
use spectacular_llms::{OpenRouterProvider, OPENROUTER_PROVIDER_ID};

pub fn provider_for_runtime(runtime: &RuntimeSelection) -> Result<OpenRouterProvider, ChatError> {
    if runtime.provider == OPENROUTER_PROVIDER_ID {
        return Ok(OpenRouterProvider::with_api_key(runtime.api_key.clone()));
    }

    Err(ChatError::Session(format!(
        "provider `{}` is not supported by chat",
        runtime.provider
    )))
}

pub fn provider_for_parts(
    provider: &str,
    api_key: String,
) -> Result<OpenRouterProvider, ChatError> {
    if provider == OPENROUTER_PROVIDER_ID {
        return Ok(OpenRouterProvider::with_api_key(api_key));
    }

    Err(ChatError::Session(format!(
        "provider `{provider}` is not supported by chat"
    )))
}
