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

/// Verifies that openai codex models advertise fast aliases with base metadata.
#[test]
fn openai_codex_models_include_fast_aliases_with_base_metadata() {
    let models = openai_codex_models();
    let base = models
        .iter()
        .find(|model| model.id() == "gpt-5.5")
        .expect("gpt-5.5 should be advertised");
    let fast = models
        .iter()
        .find(|model| model.id() == "gpt-5.5-fast")
        .expect("gpt-5.5-fast should be advertised");

    assert_eq!(fast.display_name(), "GPT-5.5 Fast");
    assert_eq!(fast.supported_parameters(), base.supported_parameters());
    assert_eq!(fast.context_window_tokens(), base.context_window_tokens());
}

/// Verifies that openai codex models advertise the gpt-5.4 fast alias.
#[test]
fn openai_codex_models_include_gpt54_fast_alias() {
    let models = openai_codex_models();
    let fast = models
        .iter()
        .find(|model| model.id() == "gpt-5.4-fast")
        .expect("gpt-5.4-fast should be advertised");

    assert_eq!(fast.display_name(), "GPT-5.4 Fast");
    assert!(fast.supports_parameter("reasoning"));
    assert!(fast.supports_parameter("tools"));
    assert_eq!(fast.context_window_tokens(), Some(128_000));
}
