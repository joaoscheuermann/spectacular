use super::ChatError;
use crate::chat::model::ChatConfigIo;
use spectacular_config::{
    CachedModelMetadata, ModelCache, ProviderConfig, ProviderModelCache, SpectacularConfig,
};
use spectacular_llms::{LlmDebugLogger, LlmProvider, Model};
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) struct ModelCacheRefresh {
    pub(super) cache: ModelCache,
    pub(super) warnings: Vec<String>,
}

struct RefreshState {
    cache: ModelCache,
    changed: bool,
    warnings: Vec<String>,
    debug_logger: LlmDebugLogger,
    now: u64,
}

struct RefreshFailure<'a> {
    provider_name: &'a str,
    cached: Option<&'a ProviderModelCache>,
    error: &'a ChatError,
    now: u64,
}

pub(super) fn refresh_single_provider_model_cache(
    config_io: ChatConfigIo,
    name: &str,
    debug_logger: LlmDebugLogger,
) -> Result<usize, ChatError> {
    let config = config_io.read_config_or_default()?;
    let provider_config = config
        .providers
        .get(name)
        .ok_or_else(|| ChatError::Session(format!("provider `{name}` is not configured")))?;
    let models = load_provider_models(provider_config, debug_logger)?;
    let count = models.len();
    let mut cache = config_io.read_model_cache_or_default()?;
    cache.put_provider(
        name.to_owned(),
        provider_config.provider_type.clone(),
        unix_timestamp(),
        models.into_iter().map(cached_model_metadata),
    );
    config_io.write_model_cache(&cache)?;
    Ok(count)
}

/// Refreshes provider model metadata and records non-fatal cache warnings.
pub(super) fn refresh_model_cache(
    config: &SpectacularConfig,
    debug_logger: LlmDebugLogger,
) -> Result<ModelCacheRefresh, ChatError> {
    let mut state = RefreshState::new(debug_logger)?;
    for (provider_name, provider_config) in &config.providers {
        state.refresh_provider(provider_name, provider_config);
    }

    state.finish()
}

impl RefreshState {
    fn new(debug_logger: LlmDebugLogger) -> Result<Self, ChatError> {
        Ok(Self {
            cache: spectacular_config::read_model_cache_or_default()?,
            changed: false,
            warnings: Vec::new(),
            debug_logger,
            now: unix_timestamp(),
        })
    }

    fn refresh_provider(&mut self, provider_name: &str, provider_config: &ProviderConfig) {
        if !provider_config.has_credentials() {
            return;
        }

        match load_provider_models(provider_config, self.debug_logger.clone()) {
            Ok(models) => self.cache_models(provider_name, provider_config, models),
            Err(error) => self
                .warnings
                .push(format_model_cache_warning(RefreshFailure {
                    provider_name,
                    cached: self.cache.provider(provider_name),
                    error: &error,
                    now: self.now,
                })),
        }
    }

    fn cache_models(
        &mut self,
        provider_name: &str,
        provider_config: &ProviderConfig,
        models: Vec<Model>,
    ) {
        self.cache.put_provider(
            provider_name.to_owned(),
            provider_config.provider_type.clone(),
            self.now,
            models.into_iter().map(cached_model_metadata),
        );
        self.changed = true;
    }

    fn finish(self) -> Result<ModelCacheRefresh, ChatError> {
        if self.changed {
            spectacular_config::write_model_cache(&self.cache)?;
        }

        Ok(ModelCacheRefresh {
            cache: self.cache,
            warnings: self.warnings,
        })
    }
}

fn load_provider_models(
    provider_config: &ProviderConfig,
    debug_logger: LlmDebugLogger,
) -> Result<Vec<Model>, ChatError> {
    let provider = crate::chat::provider::provider_for_parts(
        &provider_config.provider_type,
        provider_config.api_key().to_owned(),
        debug_logger,
    )?;

    provider
        .models(provider_config.api_key())
        .map_err(|error| ChatError::Session(error.to_string()))
}

fn cached_model_metadata(model: Model) -> CachedModelMetadata {
    CachedModelMetadata::new(
        model.id().to_owned(),
        model.display_name().to_owned(),
        model.supported_parameters().iter().cloned(),
    )
    .with_context_window_tokens(model.context_window_tokens())
}

fn format_model_cache_warning(failure: RefreshFailure<'_>) -> String {
    let Some(cached) = failure.cached else {
        return format!(
            "could not refresh model metadata for provider `{}` ({}); dynamic autocomplete is unavailable",
            failure.provider_name, failure.error
        );
    };

    stale_cache_warning(failure.provider_name, failure.error, failure.now, cached)
}

fn stale_cache_warning(
    provider_name: &str,
    error: &ChatError,
    now: u64,
    cached: &ProviderModelCache,
) -> String {
    let age = now.saturating_sub(cached.fetched_at);
    if age > 24 * 60 * 60 {
        return format!(
            "could not refresh model metadata for provider `{provider_name}` ({error}); using stale cache from {} hours ago",
            age / 3600
        );
    }

    format!(
        "could not refresh model metadata for provider `{provider_name}` ({error}); using cached API metadata"
    )
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
