use crate::chat::model::ChatConfigIo;
use crate::chat::ChatError;
use spectacular_llms::ProviderMetadata;

/// Resolves value suggestions for a command field using the current prompt context.
pub(crate) type CompletionValuesFn =
    fn(&ChatCompletionContext<'_>) -> Result<Vec<String>, ChatError>;

/// Defines whether a completed field value should be checked against its suggested values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CompletionValueValidation {
    None,
    OneOfValues,
}

/// Describes a named command field and the function that provides its value suggestions.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CompletionFieldSpec {
    pub name: &'static str,
    pub summary: &'static str,
    pub required: bool,
    pub values: CompletionValuesFn,
    pub validation: CompletionValueValidation,
}

impl CompletionFieldSpec {
    /// Returns whether this field should reject values outside its resolver output.
    pub(crate) fn validates_one_of_values(self) -> bool {
        self.validation == CompletionValueValidation::OneOfValues
    }
}

impl PartialEq for CompletionFieldSpec {
    /// Compares field metadata while intentionally ignoring resolver function identity.
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.summary == other.summary
            && self.required == other.required
            && self.validation == other.validation
    }
}

impl Eq for CompletionFieldSpec {}

/// Describes a completable subcommand and the fields it accepts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompletionSubcommandSpec {
    pub name: &'static str,
    pub summary: &'static str,
    pub fields: &'static [CompletionFieldSpec],
}

/// Describes a command with subcommand-aware completion metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompletionCommandSpec {
    pub name: &'static str,
    pub subcommands: &'static [CompletionSubcommandSpec],
}

/// Narrow read-only environment available to command-owned completion value resolvers.
#[derive(Clone, Copy)]
pub(crate) struct CompletionEnvironment {
    config_io: ChatConfigIo,
    provider_registry: &'static [ProviderMetadata],
}

impl CompletionEnvironment {
    /// Creates a completion environment from config I/O and static provider metadata.
    pub(crate) fn new(
        config_io: ChatConfigIo,
        provider_registry: &'static [ProviderMetadata],
    ) -> Self {
        Self {
            config_io,
            provider_registry,
        }
    }

    /// Returns provider backend ids enabled in the injected provider registry.
    pub(crate) fn enabled_provider_type_ids(self) -> Vec<String> {
        self.provider_registry
            .iter()
            .filter(|provider| provider.is_enabled())
            .map(|provider| provider.id().to_owned())
            .collect()
    }

    /// Returns configured provider names from persisted chat configuration.
    pub(crate) fn configured_provider_names(self) -> Result<Vec<String>, ChatError> {
        let config = self.config_io.read_config_or_default()?;
        Ok(config.providers.keys().cloned().collect())
    }

    /// Returns saved model aliases from persisted chat configuration.
    pub(crate) fn saved_model_names(self) -> Result<Vec<String>, ChatError> {
        let config = self.config_io.read_config_or_default()?;
        Ok(config.models.keys().cloned().collect())
    }

    /// Returns the configured provider for a saved model alias, when the alias exists.
    pub(crate) fn saved_model_provider(
        self,
        model_name: &str,
    ) -> Result<Option<String>, ChatError> {
        let config = self.config_io.read_config_or_default()?;
        Ok(config
            .models
            .get(model_name)
            .map(|model| model.provider.clone()))
    }

    /// Returns cached model ids scoped to one provider or deduplicated across all providers.
    pub(crate) fn cached_model_ids(self, provider: Option<&str>) -> Result<Vec<String>, ChatError> {
        let cache = self.config_io.read_model_cache_or_default()?;
        let Some(provider) = provider else {
            let mut values = cache
                .providers
                .values()
                .flat_map(|provider| provider.models.keys().cloned())
                .collect::<Vec<_>>();
            values.sort();
            values.dedup();
            return Ok(values);
        };

        Ok(cache
            .provider(provider)
            .map(|provider| provider.models.keys().cloned().collect())
            .unwrap_or_default())
    }
}

/// Runtime state available to command-owned completion value resolvers.
pub(crate) struct ChatCompletionContext<'a> {
    environment: CompletionEnvironment,
    pub subcommand: &'a str,
    pub args: CompletionArgs<'a>,
}

impl<'a> ChatCompletionContext<'a> {
    /// Creates resolver context from the narrow environment and parsed command fields.
    pub(crate) fn new(
        environment: CompletionEnvironment,
        subcommand: &'a str,
        pairs: &'a [(String, String)],
    ) -> Self {
        Self {
            environment,
            subcommand,
            args: CompletionArgs::new(pairs),
        }
    }

    /// Returns provider backend ids enabled in the active completion environment.
    pub(crate) fn enabled_provider_type_ids(&self) -> Vec<String> {
        self.environment.enabled_provider_type_ids()
    }

    /// Returns configured provider names visible to completion resolvers.
    pub(crate) fn configured_provider_names(&self) -> Result<Vec<String>, ChatError> {
        self.environment.configured_provider_names()
    }

    /// Returns saved model aliases visible to completion resolvers.
    pub(crate) fn saved_model_names(&self) -> Result<Vec<String>, ChatError> {
        self.environment.saved_model_names()
    }

    /// Returns the configured provider for a saved model alias, when it exists.
    pub(crate) fn saved_model_provider(
        &self,
        model_name: &str,
    ) -> Result<Option<String>, ChatError> {
        self.environment.saved_model_provider(model_name)
    }

    /// Returns cached model ids scoped to one provider or all configured providers.
    pub(crate) fn cached_model_ids(
        &self,
        provider: Option<&str>,
    ) -> Result<Vec<String>, ChatError> {
        self.environment.cached_model_ids(provider)
    }
}

/// Read-only view over the named arguments already typed in the current command line.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CompletionArgs<'a> {
    pairs: &'a [(String, String)],
}

impl<'a> CompletionArgs<'a> {
    /// Creates an argument view from parsed `name:value` pairs.
    pub(crate) fn new(pairs: &'a [(String, String)]) -> Self {
        Self { pairs }
    }

    /// Returns the non-empty value typed for a field, if present.
    pub(crate) fn get(self, name: &str) -> Option<&'a str> {
        self.pairs
            .iter()
            .find(|(field, _)| field == name)
            .map(|(_, value)| value.as_str())
            .filter(|value| !value.trim().is_empty())
    }
}
