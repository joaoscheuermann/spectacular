use super::{session, ChatError, RuntimeSelection};
use ::config::{DoricConfig, ModelCache, ReasoningLevel, TaskModelSlot};
use std::str::FromStr;

struct SessionModelSelection {
    provider: String,
    model: String,
    reasoning: ReasoningLevel,
}

impl RuntimeSelection {
    /// Creates the setup-only runtime used when configuration is incomplete.
    pub(crate) fn setup() -> Self {
        Self {
            provider_type: "setup".to_owned(),
            provider_auth: None,
            provider: "setup".to_owned(),
            api_key: String::new(),
            model_key: "not configured".to_owned(),
            model: "not configured".to_owned(),
            reasoning: ReasoningLevel::None,
            context_window_tokens: None,
        }
    }

    /// Returns whether this runtime can execute normal prompts.
    pub(crate) fn is_ready(&self) -> bool {
        self.provider_type != "setup"
    }

    /// Creates a runtime selection using cached provider metadata for the selected model.
    pub(crate) fn from_config_and_cache(
        config: &DoricConfig,
        cache: &ModelCache,
    ) -> Result<Self, ChatError> {
        let (model_key, coding) = config.model_for_task(TaskModelSlot::Coding)?;
        let provider_config = config.provider_for_model(model_key)?;

        Ok(Self {
            provider_type: provider_config.provider_type.clone(),
            provider_auth: provider_config.auth_mode(),
            provider: coding.provider.clone(),
            api_key: provider_config.api_key().to_owned(),
            model_key: model_key.to_owned(),
            model: coding.model.clone(),
            reasoning: coding.reasoning,
            context_window_tokens: cached_context_window_tokens(
                cache,
                &coding.provider,
                &coding.model,
            ),
        })
    }

    /// Restores runtime selection from session records and cached provider metadata.
    pub(super) fn from_session_records_and_cache(
        config: &DoricConfig,
        cache: &ModelCache,
        records: &[session::ChatRecord],
    ) -> Result<Option<Self>, ChatError> {
        let Some(selection) = latest_session_model_selection(records) else {
            return Ok(None);
        };

        Ok(runtime_from_session_selection(config, cache, selection))
    }
}

fn latest_session_model_selection(
    records: &[session::ChatRecord],
) -> Option<SessionModelSelection> {
    records.iter().rev().find_map(|record| {
        let session::ChatEvent::ModelChanged {
            slot,
            provider,
            model,
            reasoning,
            ..
        } = record.event()?
        else {
            return None;
        };

        if !is_coding_model_event(slot, provider, model) {
            return None;
        }

        Some(SessionModelSelection {
            provider: provider.clone(),
            model: model.clone(),
            reasoning: ReasoningLevel::from_str(reasoning).unwrap_or_default(),
        })
    })
}

fn is_coding_model_event(slot: &str, provider: &str, model: &str) -> bool {
    slot == TaskModelSlot::Coding.as_str()
        && !provider.trim().is_empty()
        && !model.trim().is_empty()
}

fn runtime_from_session_selection(
    config: &DoricConfig,
    cache: &ModelCache,
    selection: SessionModelSelection,
) -> Option<RuntimeSelection> {
    let provider_config = config.providers.get(&selection.provider)?;
    if !provider_config.has_credentials() {
        return None;
    }

    let model_key = session_model_key(config, &selection);
    Some(RuntimeSelection {
        provider_type: provider_config.provider_type.clone(),
        provider_auth: provider_config.auth_mode(),
        provider: selection.provider.clone(),
        api_key: provider_config.api_key().to_owned(),
        model_key,
        model: selection.model.clone(),
        reasoning: selection.reasoning,
        context_window_tokens: cached_context_window_tokens(
            cache,
            &selection.provider,
            &selection.model,
        ),
    })
}

fn session_model_key(config: &DoricConfig, selection: &SessionModelSelection) -> String {
    config
        .models
        .iter()
        .find(|(_, candidate)| {
            candidate.provider == selection.provider && candidate.model == selection.model
        })
        .map(|(key, _)| key.clone())
        .unwrap_or_else(|| config::composite_model_key(&selection.provider, &selection.model))
}

fn cached_context_window_tokens(cache: &ModelCache, provider: &str, model: &str) -> Option<usize> {
    cache
        .model(provider, model)
        .and_then(|metadata| metadata.context_window_tokens)
}
