pub const OPENROUTER_PROVIDER_ID: &str = "openrouter";

static PROVIDERS: &[ProviderMetadata] = &[ProviderMetadata::enabled(
    OPENROUTER_PROVIDER_ID,
    "OpenRouter",
)];

/// Static provider metadata used by the CLI setup flow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderMetadata {
    id: &'static str,
    display_name: &'static str,
    enabled: bool,
}

impl ProviderMetadata {
    const fn enabled(id: &'static str, display_name: &'static str) -> Self {
        Self {
            id,
            display_name,
            enabled: true,
        }
    }

    /// Stable identifier persisted in configuration.
    pub fn id(self) -> &'static str {
        self.id
    }

    /// Human-readable provider name shown in setup screens.
    pub fn display_name(self) -> &'static str {
        self.display_name
    }

    /// Whether the provider can be selected in the current release.
    pub fn is_enabled(self) -> bool {
        self.enabled
    }
}

/// Returns all providers visible in the setup UI.
pub fn provider_registry() -> &'static [ProviderMetadata] {
    PROVIDERS
}

/// Looks up provider metadata by stable identifier.
pub fn provider_by_id(provider_id: &str) -> Option<ProviderMetadata> {
    PROVIDERS
        .iter()
        .copied()
        .find(|provider| provider.id == provider_id)
}

/// Returns the only enabled provider name for placeholder setup routes.
pub fn enabled_provider_name() -> &'static str {
    PROVIDERS
        .iter()
        .find(|provider| provider.enabled)
        .map(|provider| provider.display_name)
        .unwrap_or("None")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabled_provider_is_openrouter() {
        assert_eq!(enabled_provider_name(), "OpenRouter");
    }

    #[test]
    fn registry_contains_enabled_openrouter_only() {
        let providers = provider_registry();

        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].id(), OPENROUTER_PROVIDER_ID);
        assert!(providers[0].is_enabled());
    }

    #[test]
    fn provider_lookup_uses_stable_ids() {
        let provider = provider_by_id(OPENROUTER_PROVIDER_ID).unwrap();

        assert_eq!(provider.display_name(), "OpenRouter");
    }
}
