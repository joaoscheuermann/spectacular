use spectacular_config::{ChatGptAuthConfig, ConfigError, SpectacularConfig};
use spectacular_llms::{OpenAiAuthRecord, OpenAiAuthStore, ProviderError};

use super::model::ChatConfigIo;

/// Converts persisted ChatGPT auth into provider-owned auth state.
pub(crate) fn openai_auth_record(auth: ChatGptAuthConfig) -> OpenAiAuthRecord {
    OpenAiAuthRecord {
        id_token: auth.id_token,
        access_token: auth.access_token,
        refresh_token: auth.refresh_token,
        account_id: auth.account_id,
        email: auth.email,
        plan_type: auth.plan_type,
        user_id: auth.user_id,
        fedramp: auth.fedramp,
        last_refresh_epoch_seconds: auth.last_refresh_epoch_seconds,
    }
}

/// Converts provider-owned OpenAI auth into the persisted config shape.
pub(crate) fn chatgpt_auth_config(auth: OpenAiAuthRecord) -> ChatGptAuthConfig {
    ChatGptAuthConfig {
        id_token: auth.id_token,
        access_token: auth.access_token,
        refresh_token: auth.refresh_token,
        account_id: auth.account_id,
        email: auth.email,
        plan_type: auth.plan_type,
        user_id: auth.user_id,
        fedramp: auth.fedramp,
        last_refresh_epoch_seconds: auth.last_refresh_epoch_seconds,
    }
}

/// Builds an OpenAI auth store that persists credentials through the chat config I/O seam.
pub(crate) fn config_openai_auth_store(
    provider: String,
    io: ChatConfigIo,
) -> ConfigOpenAiAuthStore {
    ConfigOpenAiAuthStore { provider, io }
}

/// OpenAI auth store backed by injected Spectacular config read/write operations.
pub(crate) struct ConfigOpenAiAuthStore {
    provider: String,
    io: ChatConfigIo,
}

impl OpenAiAuthStore for ConfigOpenAiAuthStore {
    /// Loads ChatGPT OAuth credentials from the configured provider.
    fn load_openai_auth(&self) -> Result<OpenAiAuthRecord, ProviderError> {
        let config = self
            .io
            .read_config_or_default()
            .map_err(config_auth_error)?;
        let provider = config.providers.get(&self.provider).ok_or_else(|| {
            ProviderError::AuthenticationRequired {
                provider_name: "OpenAI".to_owned(),
            }
        })?;
        let Some(auth) = provider.oauth_config() else {
            return Err(ProviderError::AuthenticationRequired {
                provider_name: "OpenAI".to_owned(),
            });
        };

        Ok(openai_auth_record(auth))
    }

    /// Saves refreshed ChatGPT OAuth credentials into the configured provider.
    fn save_openai_auth(&self, auth: OpenAiAuthRecord) -> Result<(), ProviderError> {
        let mut config = self
            .io
            .read_config_or_default()
            .map_err(config_auth_error)?;
        save_openai_auth_to_config(&mut config, &self.provider, auth).map_err(config_auth_error)?;
        self.io.write_config(&config).map_err(config_auth_error)
    }
}

/// Reports missing auth for provider instances used only for static metadata.
pub(crate) struct EmptyOpenAiAuthStore;

impl OpenAiAuthStore for EmptyOpenAiAuthStore {
    /// Reports missing auth for provider instances used only for static metadata.
    fn load_openai_auth(&self) -> Result<OpenAiAuthRecord, ProviderError> {
        Err(ProviderError::AuthenticationRequired {
            provider_name: "OpenAI".to_owned(),
        })
    }

    /// Rejects auth persistence for metadata-only provider instances.
    fn save_openai_auth(&self, _auth: OpenAiAuthRecord) -> Result<(), ProviderError> {
        Err(ProviderError::AuthenticationRequired {
            provider_name: "OpenAI".to_owned(),
        })
    }
}

/// Saves OpenAI auth into the named provider using config mutation helpers.
fn save_openai_auth_to_config(
    config: &mut SpectacularConfig,
    provider: &str,
    auth: OpenAiAuthRecord,
) -> Result<(), ConfigError> {
    config.set_provider_oauth(provider, chatgpt_auth_config(auth))
}

/// Converts config persistence errors into OpenAI auth errors.
fn config_auth_error(error: ConfigError) -> ProviderError {
    ProviderError::AuthenticationFailed {
        provider_name: "OpenAI".to_owned(),
        reason: error.to_string(),
    }
}
