pub const OPENROUTER_PROVIDER_ID: &str = "openrouter";
pub const OPENAI_PROVIDER_ID: &str = "openai";

static PROVIDERS: &[ProviderMetadata] = &[
    ProviderMetadata::enabled(OPENROUTER_PROVIDER_ID, "OpenRouter"),
    ProviderMetadata::enabled(OPENAI_PROVIDER_ID, "OpenAI"),
];

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

    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/registry.rs"
    ));
}
