use crate::Model;

/// Returns the Codex models advertised for ChatGPT-authenticated OpenAI sessions.
pub(crate) fn openai_codex_models() -> Vec<Model> {
    [
        ("gpt-5.5", "GPT-5.5"),
        ("gpt-5.5-pro", "GPT-5.5 Pro (not supported with Codex)"),
        ("gpt-5.4", "GPT-5.4"),
        ("gpt-5.4-mini", "GPT-5.4 Mini"),
        ("gpt-5.3-codex", "GPT-5.3 Codex"),
        ("gpt-5.3-codex-spark", "GPT-5.3 Codex Spark"),
        ("gpt-5.2", "GPT-5.2"),
    ]
    .into_iter()
    .map(|(id, name)| {
        Model::with_supported_parameters(id, name, ["reasoning".to_owned(), "tools".to_owned()])
    })
    .collect()
}
