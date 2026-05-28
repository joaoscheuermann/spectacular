#[test]
fn enabled_provider_is_openrouter() {
    assert_eq!(enabled_provider_name(), "OpenRouter");
}

#[test]
fn registry_contains_enabled_openrouter_and_openai() {
    let providers = provider_registry();

    assert_eq!(providers.len(), 2);
    assert_eq!(providers[0].id(), OPENROUTER_PROVIDER_ID);
    assert!(providers[0].is_enabled());
    assert_eq!(providers[1].id(), OPENAI_PROVIDER_ID);
    assert!(providers[1].is_enabled());
}

#[test]
fn provider_lookup_uses_stable_ids() {
    let provider = provider_by_id(OPENROUTER_PROVIDER_ID).unwrap();

    assert_eq!(provider.display_name(), "OpenRouter");
}
