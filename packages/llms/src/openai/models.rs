use crate::Model;

const OPENAI_LARGE_CONTEXT_WINDOW_TOKENS: usize = 128_000;
const OPENAI_LEGACY_GPT4_CONTEXT_WINDOW_TOKENS: usize = 8_192;
const OPENAI_DEFAULT_CONTEXT_WINDOW_TOKENS: usize = 32_768;
const OPENAI_CODEX_SUPPORTED_PARAMETERS: [&str; 2] = ["reasoning", "tools"];

/// Returns the Codex models advertised for ChatGPT-authenticated OpenAI sessions.
pub(crate) fn openai_codex_models() -> Vec<Model> {
    [
        ("gpt-5.5", "GPT-5.5"),
        ("gpt-5.5-fast", "GPT-5.5 Fast"),
        ("gpt-5.5-pro", "GPT-5.5 Pro (not supported with Codex)"),
        ("gpt-5.4", "GPT-5.4"),
        ("gpt-5.4-fast", "GPT-5.4 Fast"),
        ("gpt-5.4-mini", "GPT-5.4 Mini"),
        ("gpt-5.3-codex", "GPT-5.3 Codex"),
        ("gpt-5.3-codex-spark", "GPT-5.3 Codex Spark"),
        ("gpt-5.2", "GPT-5.2"),
    ]
    .into_iter()
    .map(openai_codex_model)
    .collect()
}

/// Builds one OpenAI Codex model with shared provider metadata.
fn openai_codex_model((id, name): (&str, &str)) -> Model {
    Model::with_supported_parameters(
        id,
        name,
        OPENAI_CODEX_SUPPORTED_PARAMETERS
            .into_iter()
            .map(str::to_owned),
    )
    .with_context_window_tokens(openai_context_window_tokens(id))
}

/// Resolves known OpenAI model context windows, using a conservative provider-owned fallback.
pub(crate) fn openai_context_window_tokens(model: &str) -> Option<usize> {
    let model = model.trim().to_ascii_lowercase();
    if model.is_empty() {
        return None;
    }

    if model.contains("gpt-4o")
        || model.contains("gpt-4.1")
        || model.contains("gpt-4-turbo")
        || model.contains("gpt-5")
        || model.contains("o1")
        || model.contains("o3")
        || model.contains("o4")
    {
        return Some(OPENAI_LARGE_CONTEXT_WINDOW_TOKENS);
    }

    if model.contains("gpt-4-32k") {
        return Some(OPENAI_DEFAULT_CONTEXT_WINDOW_TOKENS);
    }

    if model.contains("gpt-4") {
        return Some(OPENAI_LEGACY_GPT4_CONTEXT_WINDOW_TOKENS);
    }

    Some(OPENAI_DEFAULT_CONTEXT_WINDOW_TOKENS)
}
