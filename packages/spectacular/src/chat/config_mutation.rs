use super::auth::chatgpt_auth_config;
use super::{ChatError, RuntimeSelection};
use crate::chat::model::{ChatConfigIo, ChatModel};
use spectacular_config::{CachedModelMetadata, ReasoningLevel, SpectacularConfig, TaskModelSlot};
use spectacular_llms::{LlmProvider, OpenAiAuthRecord};

impl ChatModel {
    /// Stores API-key credentials for the canonical provider type entry.
    pub fn set_provider_api_key(
        &mut self,
        provider_type: &str,
        apikey: &str,
    ) -> Result<(), ChatError> {
        update_config(self.config_io(), |config| {
            config.set_provider_api_key(provider_type, apikey)?;
            Ok(())
        })
    }

    /// Validates that a provider type can use ChatGPT browser authentication.
    pub fn validate_openai_auth_provider(&self, provider_type: &str) -> Result<(), ChatError> {
        if provider_type == spectacular_llms::OPENAI_PROVIDER_ID {
            return Ok(());
        }

        Err(ChatError::Session(format!(
            "/provider auth supports only provider type `openai`; got `{provider_type}`"
        )))
    }

    /// Stores ChatGPT OAuth credentials into the canonical OpenAI provider entry.
    pub fn set_openai_provider_auth(
        &mut self,
        provider_type: &str,
        auth: OpenAiAuthRecord,
    ) -> Result<(), ChatError> {
        update_config(self.config_io(), |config| {
            config.set_provider_oauth(provider_type, chatgpt_auth_config(auth))?;
            Ok(())
        })
    }

    /// Refreshes cached model metadata for a single configured provider.
    pub fn refresh_provider_model_cache(&self, name: &str) -> Result<usize, ChatError> {
        let config_io = self.config_io();
        let config = config_io.read_config_or_default()?;
        let provider_config = config
            .providers
            .get(name)
            .ok_or_else(|| ChatError::Session(format!("provider `{name}` is not configured")))?;
        let provider = crate::chat::provider::provider_for_parts(
            &provider_config.provider_type,
            provider_config.api_key().to_owned(),
            self.debug_logger().clone(),
        )?;
        let models = provider
            .models(provider_config.api_key())
            .map_err(|error| ChatError::Session(error.to_string()))?;
        let count = models.len();
        let mut cache = config_io.read_model_cache_or_default()?;
        cache.put_provider(
            name.to_owned(),
            provider_config.provider_type.clone(),
            crate::chat::unix_timestamp(),
            models.into_iter().map(|model| {
                CachedModelMetadata::new(
                    model.id().to_owned(),
                    model.display_name().to_owned(),
                    model.supported_parameters().iter().cloned(),
                )
            }),
        );
        config_io.write_model_cache(&cache)?;
        Ok(count)
    }

    /// Removes a provider after backing up config and returns model keys that reference it.
    pub fn remove_provider(&mut self, name: &str) -> Result<Vec<String>, ChatError> {
        let config_io = self.config_io();
        let mut config = config_io.read_config_or_default()?;
        let orphaned_models = config
            .models
            .iter()
            .filter(|(_, model)| model.provider == name)
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        config_io.backup_config()?;
        config.remove_provider(name)?;
        config_io.write_config(&config)?;
        Ok(orphaned_models)
    }

    /// Adds a model definition to config and returns the saved model key.
    pub fn add_model(
        &mut self,
        provider: &str,
        model_id: &str,
        reasoning: ReasoningLevel,
        name: Option<String>,
    ) -> Result<String, ChatError> {
        let mut saved_key = String::new();
        update_config(self.config_io(), |config| {
            saved_key = config.add_model(provider, model_id, reasoning, name)?;
            Ok(())
        })?;
        Ok(saved_key)
    }

    /// Edits a saved model and refreshes the active runtime when the coding model changed.
    pub fn edit_model(
        &mut self,
        name: &str,
        provider: Option<String>,
        model_id: Option<String>,
        reasoning: Option<ReasoningLevel>,
    ) -> Result<Option<RuntimeSelection>, ChatError> {
        let mut config = self.config_io().read_config_or_default()?;
        config.edit_model(name, provider, model_id, reasoning)?;
        self.config_io().write_config(&config)?;
        self.refresh_runtime_if_coding_model_changed(&config, name)
    }

    /// Removes a saved model after backing up config and returns task slots that referenced it.
    pub fn remove_model(&mut self, name: &str) -> Result<Vec<TaskModelSlot>, ChatError> {
        let config_io = self.config_io();
        let mut config = config_io.read_config_or_default()?;
        let references = config.tasks.references_to(name);
        config_io.backup_config()?;
        config.remove_model(name)?;
        config_io.write_config(&config)?;
        Ok(references)
    }

    /// Assigns a model to a task slot and refreshes the runtime for coding-slot changes.
    pub fn set_task_model(
        &mut self,
        slot: TaskModelSlot,
        model_key: &str,
    ) -> Result<Option<RuntimeSelection>, ChatError> {
        let mut config = self.config_io().read_config_or_default()?;
        config.set_task_model(slot, model_key)?;
        self.config_io().write_config(&config)?;
        if slot != TaskModelSlot::Coding {
            return Ok(None);
        }

        let runtime = RuntimeSelection::from_config(&config)?;
        self.replace_runtime(runtime.clone());
        self.append_runtime_defaults("command")?;
        Ok(Some(runtime))
    }
}

/// Loads, mutates, and writes config through the injected chat config I/O seam.
fn update_config(
    config_io: ChatConfigIo,
    mutate: impl FnOnce(&mut SpectacularConfig) -> Result<(), ChatError>,
) -> Result<(), ChatError> {
    let mut config = config_io.read_config_or_default()?;
    mutate(&mut config)?;
    config_io.write_config(&config)?;
    Ok(())
}
