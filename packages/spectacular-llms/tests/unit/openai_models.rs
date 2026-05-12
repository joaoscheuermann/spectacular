/// Verifies that openai provider resolves known model context windows.
#[test]
fn openai_provider_resolves_known_model_context_windows() {
    let provider = OpenAiProvider::with_api_key("sk-test");

    assert_eq!(provider.context_window_tokens("gpt-5.5"), Some(128_000));
    assert_eq!(provider.context_window_tokens("gpt-4o"), Some(128_000));
    assert_eq!(provider.context_window_tokens("gpt-4-32k"), Some(32_768));
    assert_eq!(provider.context_window_tokens("gpt-4"), Some(8_192));
}

/// Verifies that openai codex models include context window metadata.
#[test]
fn openai_codex_models_include_context_window_metadata() {
    let model = openai_codex_models()
        .into_iter()
        .find(|model| model.id() == "gpt-5.5")
        .expect("gpt-5.5 should be advertised");

    assert_eq!(model.context_window_tokens(), Some(128_000));
}
