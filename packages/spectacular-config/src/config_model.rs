impl SpectacularConfig {
    /// Applies the add provider operation to the persisted Spectacular configuration.
    pub fn add_provider(
        &mut self,
        name: impl Into<String>,
        provider_type: impl Into<String>,
        apikey: impl Into<String>,
    ) -> Result<(), ConfigError> {
        let name = require_text("provider name", name.into())?;
        let provider_type = require_text("provider type", provider_type.into())?;
        let apikey = require_text("api key", apikey.into())?;

        if self.providers.contains_key(&name) {
            return Err(ConfigError::ProviderAlreadyExists { provider: name });
        }

        self.providers
            .insert(name, ProviderConfig::new(provider_type, apikey));
        Ok(())
    }

    /// Applies the remove provider operation to the persisted Spectacular configuration.
    pub fn remove_provider(&mut self, name: &str) -> Result<ProviderConfig, ConfigError> {
        self.providers
            .remove(name)
            .ok_or_else(|| ConfigError::ProviderNotConfigured {
                provider: name.to_owned(),
            })
    }

    /// Applies the add model operation to the persisted Spectacular configuration.
    pub fn add_model(
        &mut self,
        provider: impl Into<String>,
        model: impl Into<String>,
        reasoning: ReasoningLevel,
        name: Option<String>,
    ) -> Result<String, ConfigError> {
        let provider = require_text("provider", provider.into())?;
        self.require_provider(&provider)?;
        let model = require_text("model", model.into())?;
        let key = match name {
            Some(name) => require_text("model name", name)?,
            None => composite_model_key(&provider, &model),
        };

        if self.models.contains_key(&key) {
            return Err(ConfigError::ModelAlreadyExists { model: key });
        }

        self.models
            .insert(key.clone(), ModelConfig::new(provider, model, reasoning));
        Ok(key)
    }

    /// Applies the edit model operation to the persisted Spectacular configuration.
    pub fn edit_model(
        &mut self,
        name: &str,
        provider: Option<String>,
        model: Option<String>,
        reasoning: Option<ReasoningLevel>,
    ) -> Result<(), ConfigError> {
        if let Some(provider) = provider.as_ref() {
            self.require_provider(provider)?;
        }

        let current = self
            .models
            .get_mut(name)
            .ok_or_else(|| ConfigError::ModelNotConfigured {
                model: name.to_owned(),
            })?;

        if let Some(provider) = provider {
            current.provider = require_text("provider", provider)?;
        }
        if let Some(model) = model {
            current.model = require_text("model", model)?;
        }
        if let Some(reasoning) = reasoning {
            current.reasoning = reasoning;
        }

        current.internal_key = composite_model_key(&current.provider, &current.model);
        Ok(())
    }

    /// Applies the remove model operation to the persisted Spectacular configuration.
    pub fn remove_model(&mut self, name: &str) -> Result<ModelConfig, ConfigError> {
        self.models
            .remove(name)
            .ok_or_else(|| ConfigError::ModelNotConfigured {
                model: name.to_owned(),
            })
    }

    /// Applies the set task model operation to the persisted Spectacular configuration.
    pub fn set_task_model(
        &mut self,
        slot: TaskModelSlot,
        model_key: impl Into<String>,
    ) -> Result<(), ConfigError> {
        let model_key = require_text("model", model_key.into())?;
        if !self.models.contains_key(&model_key) {
            return Err(ConfigError::ModelNotConfigured { model: model_key });
        }

        self.tasks.set(slot, model_key);
        Ok(())
    }

    /// Applies the validate complete operation to the persisted Spectacular configuration.
    pub fn validate_complete(&self) -> Result<(), ConfigError> {
        for slot in TaskModelSlot::ALL {
            let model_key = required_text(self.tasks.get(slot))
                .ok_or(ConfigError::MissingTaskModel { slot })?;
            let model = self.models.get(model_key).ok_or_else(|| {
                ConfigError::InvalidTaskModelReference {
                    slot,
                    model: model_key.to_owned(),
                }
            })?;
            self.validate_model_provider(model_key, model)?;
        }

        Ok(())
    }

    /// Applies the is complete operation to the persisted Spectacular configuration.
    pub fn is_complete(&self) -> bool {
        self.validate_complete().is_ok()
    }

    /// Applies the model for task operation to the persisted Spectacular configuration.
    pub fn model_for_task(&self, slot: TaskModelSlot) -> Result<(&str, &ModelConfig), ConfigError> {
        let model_key =
            required_text(self.tasks.get(slot)).ok_or(ConfigError::MissingTaskModel { slot })?;
        let model =
            self.models
                .get(model_key)
                .ok_or_else(|| ConfigError::InvalidTaskModelReference {
                    slot,
                    model: model_key.to_owned(),
                })?;
        self.validate_model_provider(model_key, model)?;
        Ok((model_key, model))
    }

    /// Applies the provider for model operation to the persisted Spectacular configuration.
    pub fn provider_for_model(&self, model_key: &str) -> Result<&ProviderConfig, ConfigError> {
        let model = self
            .models
            .get(model_key)
            .ok_or_else(|| ConfigError::ModelNotConfigured {
                model: model_key.to_owned(),
            })?;
        self.validate_model_provider(model_key, model)
    }

    /// Applies the require provider operation to the persisted Spectacular configuration.
    fn require_provider(&self, provider: &str) -> Result<&ProviderConfig, ConfigError> {
        self.providers
            .get(provider)
            .ok_or_else(|| ConfigError::ProviderNotConfigured {
                provider: provider.to_owned(),
            })
    }

    /// Applies the validate model provider operation to the persisted Spectacular configuration.
    fn validate_model_provider(
        &self,
        model_key: &str,
        model: &ModelConfig,
    ) -> Result<&ProviderConfig, ConfigError> {
        let provider = self.providers.get(&model.provider).ok_or_else(|| {
            ConfigError::ModelProviderNotConfigured {
                model: model_key.to_owned(),
                provider: model.provider.clone(),
            }
        })?;

        if required_text(Some(provider.apikey.as_str())).is_none() {
            return Err(ConfigError::MissingProviderApiKey {
                provider: model.provider.clone(),
            });
        }

        if required_text(Some(provider.provider_type.as_str())).is_none() {
            return Err(ConfigError::InvalidProviderType {
                provider: model.provider.clone(),
            });
        }

        Ok(provider)
    }
}
