/// Reasoning effort persisted for a saved model.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningLevel {
    #[default]
    None,
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
}
impl ReasoningLevel {
    pub const ALL: [ReasoningLevel; 6] = [
        ReasoningLevel::None,
        ReasoningLevel::Minimal,
        ReasoningLevel::Low,
        ReasoningLevel::Medium,
        ReasoningLevel::High,
        ReasoningLevel::Xhigh,
    ];

    /// Provides the as str behavior for persisted configuration values.
    pub fn as_str(self) -> &'static str {
        match self {
            ReasoningLevel::None => "none",
            ReasoningLevel::Minimal => "minimal",
            ReasoningLevel::Low => "low",
            ReasoningLevel::Medium => "medium",
            ReasoningLevel::High => "high",
            ReasoningLevel::Xhigh => "xhigh",
        }
    }

    /// Provides the non none behavior for persisted configuration values.
    pub fn non_none(self) -> bool {
        self != Self::None
    }
}

impl FromStr for ReasoningLevel {
    type Err = ConfigParseError;

    /// Provides the from str behavior for persisted configuration values.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "none" => Ok(ReasoningLevel::None),
            "minimal" => Ok(ReasoningLevel::Minimal),
            "low" => Ok(ReasoningLevel::Low),
            "medium" => Ok(ReasoningLevel::Medium),
            "high" => Ok(ReasoningLevel::High),
            "xhigh" => Ok(ReasoningLevel::Xhigh),
            _ => Err(ConfigParseError::InvalidReasoning {
                value: value.to_owned(),
            }),
        }
    }
}

impl Display for ReasoningLevel {
    /// Provides the fmt behavior for persisted configuration values.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Canonical task slots that must point at saved model keys.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct TaskAssignments {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub general: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labeling: Option<String>,
}

impl TaskAssignments {
    /// Provides the get behavior for persisted configuration values.
    pub fn get(&self, slot: TaskModelSlot) -> Option<&str> {
        match slot {
            TaskModelSlot::General => self.general.as_deref(),
            TaskModelSlot::Coding => self.coding.as_deref(),
            TaskModelSlot::Labeling => self.labeling.as_deref(),
        }
    }

    /// Provides the set behavior for persisted configuration values.
    pub fn set(&mut self, slot: TaskModelSlot, model_key: impl Into<String>) {
        match slot {
            TaskModelSlot::General => self.general = Some(model_key.into()),
            TaskModelSlot::Coding => self.coding = Some(model_key.into()),
            TaskModelSlot::Labeling => self.labeling = Some(model_key.into()),
        }
    }

    /// Provides the references to behavior for persisted configuration values.
    pub fn references_to(&self, model_key: &str) -> Vec<TaskModelSlot> {
        TaskModelSlot::ALL
            .into_iter()
            .filter(|slot| self.get(*slot) == Some(model_key))
            .collect()
    }
}

/// Persisted configuration for one saved model alias.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ModelConfig {
    pub provider: String,
    pub model: String,
    pub reasoning: ReasoningLevel,
    pub internal_key: String,
}

impl ModelConfig {
    /// Provides the new behavior for persisted configuration values.
    pub fn new(
        provider: impl Into<String>,
        model: impl Into<String>,
        reasoning: ReasoningLevel,
    ) -> Self {
        let provider = provider.into();
        let model = model.into();
        let internal_key = composite_model_key(&provider, &model);
        Self {
            provider,
            model,
            reasoning,
            internal_key,
        }
    }

    /// Provides the with internal key behavior for persisted configuration values.
    pub fn with_internal_key(mut self) -> Self {
        self.internal_key = composite_model_key(&self.provider, &self.model);
        self
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct SpectacularConfig {
    pub providers: BTreeMap<String, ProviderConfig>,
    pub models: BTreeMap<String, ModelConfig>,
    pub tasks: TaskAssignments,
}

impl<'de> Deserialize<'de> for SpectacularConfig {
    /// Provides the deserialize behavior for persisted configuration values.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        if looks_like_legacy_schema(&value) {
            return Err(serde::de::Error::custom(SCHEMA_CHANGED_MESSAGE));
        }

        let wire = SpectacularConfigWire::deserialize(value).map_err(serde::de::Error::custom)?;
        Ok(Self {
            providers: wire.providers,
            models: wire
                .models
                .into_iter()
                .map(|(key, model)| (key, model.with_internal_key()))
                .collect(),
            tasks: wire.tasks,
        })
    }
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct SpectacularConfigWire {
    providers: BTreeMap<String, ProviderConfig>,
    models: BTreeMap<String, ModelConfig>,
    tasks: TaskAssignments,
}

/// Cached API metadata used for strict model discovery while offline.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ModelCache {
    pub providers: BTreeMap<String, ProviderModelCache>,
}

impl ModelCache {
    /// Provides the provider behavior for persisted configuration values.
    pub fn provider(&self, provider_name: &str) -> Option<&ProviderModelCache> {
        self.providers.get(provider_name)
    }

    /// Provides the model behavior for persisted configuration values.
    pub fn model(&self, provider_name: &str, model_id: &str) -> Option<&CachedModelMetadata> {
        self.provider(provider_name)?.models.get(model_id)
    }

    /// Provides the put provider behavior for persisted configuration values.
    pub fn put_provider(
        &mut self,
        provider_name: impl Into<String>,
        provider_type: impl Into<String>,
        fetched_at: u64,
        models: impl IntoIterator<Item = CachedModelMetadata>,
    ) {
        self.providers.insert(
            provider_name.into(),
            ProviderModelCache {
                provider_type: provider_type.into(),
                fetched_at,
                models: models
                    .into_iter()
                    .map(|model| (model.id.clone(), model))
                    .collect(),
            },
        );
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProviderModelCache {
    pub provider_type: String,
    pub fetched_at: u64,
    pub models: BTreeMap<String, CachedModelMetadata>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct CachedModelMetadata {
    pub id: String,
    pub name: String,
    pub supported_parameters: Vec<String>,
}

impl CachedModelMetadata {
    /// Provides the new behavior for persisted configuration values.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        supported_parameters: impl IntoIterator<Item = String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            supported_parameters: supported_parameters.into_iter().collect(),
        }
    }

    /// Provides the supports parameter behavior for persisted configuration values.
    pub fn supports_parameter(&self, parameter: &str) -> bool {
        self.supported_parameters
            .iter()
            .any(|candidate| candidate == parameter)
    }

    /// Provides the supports reasoning behavior for persisted configuration values.
    pub fn supports_reasoning(&self) -> bool {
        self.supports_parameter("reasoning")
    }
}
