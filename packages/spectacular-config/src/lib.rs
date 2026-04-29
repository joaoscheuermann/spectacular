use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::ffi::OsString;
use std::fmt::{self, Display};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;

const APP_CONFIG_DIR_NAME: &str = "spectacular";
const CONFIG_FILE_NAME: &str = "config.json";

/// Reasoning effort persisted for a task model assignment.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningLevel {
    #[default]
    None,
    Low,
    Medium,
    High,
}

impl ReasoningLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            ReasoningLevel::None => "none",
            ReasoningLevel::Low => "low",
            ReasoningLevel::Medium => "medium",
            ReasoningLevel::High => "high",
        }
    }
}

impl FromStr for ReasoningLevel {
    type Err = ConfigParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "none" => Ok(ReasoningLevel::None),
            "low" => Ok(ReasoningLevel::Low),
            "medium" => Ok(ReasoningLevel::Medium),
            "high" => Ok(ReasoningLevel::High),
            _ => Err(ConfigParseError::InvalidReasoning {
                value: value.to_owned(),
            }),
        }
    }
}

impl Display for ReasoningLevel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Task model identifiers required before commands can use LLM-backed behavior.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct TaskModels {
    pub planning: Option<TaskModelConfig>,
    pub labeling: Option<TaskModelConfig>,
    pub coding: Option<TaskModelConfig>,
}

/// Persisted model and reasoning assignment for a task slot.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct TaskModelConfig {
    pub model: String,
    pub reasoning: ReasoningLevel,
}

impl TaskModelConfig {
    pub fn new(model: impl Into<String>, reasoning: ReasoningLevel) -> Self {
        Self {
            model: model.into(),
            reasoning,
        }
    }
}

impl Default for TaskModelConfig {
    fn default() -> Self {
        Self {
            model: String::new(),
            reasoning: ReasoningLevel::None,
        }
    }
}

/// Persisted configuration for one provider.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct ProviderConfig {
    pub key: Option<String>,
    pub tasks: TaskModels,
}

/// Persisted provider configuration namespace.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct ProvidersConfig {
    pub selected: Option<String>,
    pub available: BTreeMap<String, ProviderConfig>,
}

/// Persisted Spectacular CLI configuration.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct SpectacularConfig {
    pub providers: ProvidersConfig,
}

impl<'de> Deserialize<'de> for SpectacularConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = SpectacularConfigWire::deserialize(deserializer)?;

        if let Some(providers) = wire.providers {
            return Ok(Self { providers });
        }

        Ok(Self::from_legacy(wire))
    }
}

/// Returns the platform-specific Spectacular configuration directory.
pub fn config_dir() -> Result<PathBuf, ConfigError> {
    config_dir_with_env(|key| env::var_os(key))
}

/// Returns the platform-specific `config.json` path.
pub fn config_path() -> Result<PathBuf, ConfigError> {
    Ok(config_dir()?.join(CONFIG_FILE_NAME))
}

/// Reads `config.json` from the platform-specific app config directory.
pub fn read_config() -> Result<SpectacularConfig, ConfigError> {
    read_config_from_path(&config_path()?)
}

/// Reads `config.json`, returning an empty config when the file does not exist.
pub fn read_config_or_default() -> Result<SpectacularConfig, ConfigError> {
    let path = config_path()?;

    if !path.exists() {
        return Ok(SpectacularConfig::default());
    }

    read_config_from_path(&path)
}

/// Writes `config.json` to the platform-specific app config directory.
pub fn write_config(config: &SpectacularConfig) -> Result<(), ConfigError> {
    let path = config_path()?;
    write_config_to_path(&path, config)
}

/// Formats config as pretty JSON using the persisted schema.
pub fn to_pretty_json(config: &SpectacularConfig) -> Result<String, ConfigError> {
    serde_json::to_string_pretty(config).map_err(|source| ConfigError::SerializeFailed {
        path: config_path().unwrap_or_else(|_| PathBuf::from(CONFIG_FILE_NAME)),
        source,
    })
}

/// Reads config from a caller-provided path. Useful for tests and future setup flows.
pub fn read_config_from_path(path: &Path) -> Result<SpectacularConfig, ConfigError> {
    let content = fs::read_to_string(path).map_err(|source| {
        if source.kind() == io::ErrorKind::NotFound {
            ConfigError::MissingConfigFile {
                path: path.to_path_buf(),
            }
        } else {
            ConfigError::ReadFailed {
                path: path.to_path_buf(),
                source,
            }
        }
    })?;

    let json = content.trim_start_matches('\u{feff}');

    serde_json::from_str(json).map_err(|source| ConfigError::InvalidJson {
        path: path.to_path_buf(),
        source,
    })
}

/// Writes config to a caller-provided path, creating parent directories as needed.
pub fn write_config_to_path(path: &Path, config: &SpectacularConfig) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| ConfigError::WriteFailed {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let content =
        serde_json::to_string_pretty(config).map_err(|source| ConfigError::SerializeFailed {
            path: path.to_path_buf(),
            source,
        })?;

    fs::write(path, content).map_err(|source| ConfigError::WriteFailed {
        path: path.to_path_buf(),
        source,
    })
}

impl SpectacularConfig {
    /// Updates the active provider identifier persisted by the CLI.
    pub fn select_provider(&mut self, provider_id: impl Into<String>) -> Result<(), ConfigError> {
        let provider_id = provider_id.into();

        if !self.providers.available.contains_key(&provider_id) {
            return Err(ConfigError::ProviderNotConfigured {
                provider: provider_id,
            });
        }

        self.providers.selected = Some(provider_id);
        Ok(())
    }

    /// Replaces the stored API key for a provider without touching other provider keys.
    pub fn set_provider_api_key(
        &mut self,
        provider_id: impl Into<String>,
        api_key: impl Into<String>,
    ) {
        self.provider_entry(provider_id.into()).key = Some(api_key.into());
    }

    /// Saves one task model assignment for a provider.
    pub fn set_provider_task_model(
        &mut self,
        provider_id: impl Into<String>,
        slot: TaskModelSlot,
        model: impl Into<String>,
        reasoning: ReasoningLevel,
    ) {
        let task_model = TaskModelConfig::new(model, reasoning);
        let provider = self.provider_entry(provider_id.into());

        match slot {
            TaskModelSlot::Planning => provider.tasks.planning = Some(task_model),
            TaskModelSlot::Labeling => provider.tasks.labeling = Some(task_model),
            TaskModelSlot::Coding => provider.tasks.coding = Some(task_model),
        }
    }

    /// Validates that all required configuration fields are present and non-empty.
    pub fn validate_complete(&self) -> Result<(), ConfigError> {
        let provider = required_text(self.providers.selected.as_deref())
            .ok_or(ConfigError::NoSelectedProvider)?;
        let provider_config = self.providers.available.get(provider).ok_or_else(|| {
            ConfigError::ProviderNotConfigured {
                provider: provider.to_owned(),
            }
        })?;

        if required_text(provider_config.key.as_deref()).is_none() {
            return Err(ConfigError::MissingProviderApiKey {
                provider: provider.to_owned(),
            });
        }

        require_task_model(
            TaskModelSlot::Planning,
            provider_config.tasks.planning.as_ref(),
        )?;
        require_task_model(
            TaskModelSlot::Labeling,
            provider_config.tasks.labeling.as_ref(),
        )?;
        require_task_model(TaskModelSlot::Coding, provider_config.tasks.coding.as_ref())?;

        Ok(())
    }

    /// Returns true when all required setup values are present and non-empty.
    pub fn is_complete(&self) -> bool {
        self.validate_complete().is_ok()
    }

    fn provider_entry(&mut self, provider_id: String) -> &mut ProviderConfig {
        self.providers.available.entry(provider_id).or_default()
    }

    fn from_legacy(wire: SpectacularConfigWire) -> Self {
        let mut config = SpectacularConfig::default();

        for (provider_id, api_key) in wire.provider_api_keys {
            config.set_provider_api_key(provider_id, api_key);
        }

        if let Some(provider_id) = wire.selected_provider {
            config.providers.selected = Some(provider_id.clone());
            config.provider_entry(provider_id.clone());

            if let Some(planning) = wire
                .task_models
                .planning
                .and_then(LegacyTaskModel::into_config)
            {
                config.set_provider_task_model(
                    provider_id.clone(),
                    TaskModelSlot::Planning,
                    planning.model,
                    planning.reasoning,
                );
            }

            if let Some(labeling) = wire
                .task_models
                .labeling
                .and_then(LegacyTaskModel::into_config)
            {
                config.set_provider_task_model(
                    provider_id.clone(),
                    TaskModelSlot::Labeling,
                    labeling.model,
                    labeling.reasoning,
                );
            }

            if let Some(coding) = wire
                .task_models
                .coding
                .and_then(LegacyTaskModel::into_config)
            {
                config.set_provider_task_model(
                    provider_id,
                    TaskModelSlot::Coding,
                    coding.model,
                    coding.reasoning,
                );
            }
        }

        config
    }
}

/// Errors returned by config path, persistence, and completeness operations.
#[derive(Debug)]
pub enum ConfigError {
    ConfigDirUnavailable,
    MissingConfigFile {
        path: PathBuf,
    },
    InvalidJson {
        path: PathBuf,
        source: serde_json::Error,
    },
    ReadFailed {
        path: PathBuf,
        source: io::Error,
    },
    WriteFailed {
        path: PathBuf,
        source: io::Error,
    },
    SerializeFailed {
        path: PathBuf,
        source: serde_json::Error,
    },
    NoSelectedProvider,
    ProviderNotConfigured {
        provider: String,
    },
    MissingProviderApiKey {
        provider: String,
    },
    MissingTaskModel {
        slot: TaskModelSlot,
    },
    InvalidTaskModel {
        slot: TaskModelSlot,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskModelSlot {
    Planning,
    Labeling,
    Coding,
}

impl TaskModelSlot {
    pub fn as_str(self) -> &'static str {
        match self {
            TaskModelSlot::Planning => "planning",
            TaskModelSlot::Labeling => "labeling",
            TaskModelSlot::Coding => "coding",
        }
    }
}

impl FromStr for TaskModelSlot {
    type Err = ConfigParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "planning" => Ok(TaskModelSlot::Planning),
            "labeling" => Ok(TaskModelSlot::Labeling),
            "coding" => Ok(TaskModelSlot::Coding),
            _ => Err(ConfigParseError::InvalidTask {
                value: value.to_owned(),
            }),
        }
    }
}

impl Display for TaskModelSlot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug)]
pub enum ConfigParseError {
    InvalidReasoning { value: String },
    InvalidTask { value: String },
}

impl Display for ConfigParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigParseError::InvalidReasoning { value } => write!(
                formatter,
                "invalid reasoning `{value}`; expected one of: none, low, medium, high"
            ),
            ConfigParseError::InvalidTask { value } => write!(
                formatter,
                "invalid task `{value}`; expected one of: planning, labeling, coding"
            ),
        }
    }
}

impl Error for ConfigParseError {}

impl Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::ConfigDirUnavailable => {
                formatter.write_str("could not resolve the Spectacular config directory")
            }
            ConfigError::MissingConfigFile { path } => {
                write!(
                    formatter,
                    "config file does not exist at {}",
                    path.display()
                )
            }
            ConfigError::InvalidJson { path, .. } => {
                write!(
                    formatter,
                    "config file contains invalid JSON at {}",
                    path.display()
                )
            }
            ConfigError::ReadFailed { path, .. } => {
                write!(
                    formatter,
                    "failed to read config file at {}",
                    path.display()
                )
            }
            ConfigError::WriteFailed { path, .. } => {
                write!(
                    formatter,
                    "failed to write config file at {}",
                    path.display()
                )
            }
            ConfigError::SerializeFailed { path, .. } => {
                write!(
                    formatter,
                    "failed to serialize config file at {}",
                    path.display()
                )
            }
            ConfigError::NoSelectedProvider => formatter.write_str("no provider is selected"),
            ConfigError::ProviderNotConfigured { provider } => {
                write!(formatter, "provider `{provider}` is not configured")
            }
            ConfigError::MissingProviderApiKey { provider } => {
                write!(formatter, "provider `{provider}` does not have an API key")
            }
            ConfigError::MissingTaskModel { slot } => {
                write!(formatter, "missing `{slot}` model assignment")
            }
            ConfigError::InvalidTaskModel { slot } => {
                write!(formatter, "`{slot}` model assignment is invalid")
            }
        }
    }
}

impl Error for ConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ConfigError::InvalidJson { source, .. }
            | ConfigError::SerializeFailed { source, .. } => Some(source),
            ConfigError::ReadFailed { source, .. } | ConfigError::WriteFailed { source, .. } => {
                Some(source)
            }
            ConfigError::ConfigDirUnavailable
            | ConfigError::MissingConfigFile { .. }
            | ConfigError::NoSelectedProvider
            | ConfigError::ProviderNotConfigured { .. }
            | ConfigError::MissingProviderApiKey { .. }
            | ConfigError::MissingTaskModel { .. }
            | ConfigError::InvalidTaskModel { .. } => None,
        }
    }
}

fn require_task_model(
    slot: TaskModelSlot,
    value: Option<&TaskModelConfig>,
) -> Result<(), ConfigError> {
    let Some(task_model) = value else {
        return Err(ConfigError::MissingTaskModel { slot });
    };

    if required_text(Some(task_model.model.as_str())).is_some() {
        return Ok(());
    }

    Err(ConfigError::InvalidTaskModel { slot })
}

fn required_text(value: Option<&str>) -> Option<&str> {
    value.filter(|text| !text.trim().is_empty())
}

fn config_dir_with_env(get_env: impl Fn(&str) -> Option<OsString>) -> Result<PathBuf, ConfigError> {
    platform_config_base(get_env)
        .map(|base| base.join(APP_CONFIG_DIR_NAME))
        .ok_or(ConfigError::ConfigDirUnavailable)
}

#[cfg(windows)]
fn platform_config_base(get_env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    get_env("APPDATA").map(PathBuf::from)
}

#[cfg(target_os = "macos")]
fn platform_config_base(get_env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    get_env("HOME")
        .map(PathBuf::from)
        .map(|home| home.join("Library").join("Application Support"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_config_base(get_env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    get_env("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| {
        get_env("HOME")
            .map(PathBuf::from)
            .map(|home| home.join(".config"))
    })
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct SpectacularConfigWire {
    providers: Option<ProvidersConfig>,
    selected_provider: Option<String>,
    provider_api_keys: BTreeMap<String, String>,
    task_models: LegacyTaskModels,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct LegacyTaskModels {
    planning: Option<LegacyTaskModel>,
    labeling: Option<LegacyTaskModel>,
    coding: Option<LegacyTaskModel>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum LegacyTaskModel {
    Model(String),
    Config(TaskModelConfig),
}

impl LegacyTaskModel {
    fn into_config(self) -> Option<TaskModelConfig> {
        match self {
            LegacyTaskModel::Model(model) => {
                if required_text(Some(model.as_str())).is_some() {
                    return Some(TaskModelConfig::new(model, ReasoningLevel::None));
                }

                None
            }
            LegacyTaskModel::Config(config) => Some(config),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn default_config_is_incomplete_without_provider() {
        let error = SpectacularConfig::default()
            .validate_complete()
            .unwrap_err();

        assert!(matches!(error, ConfigError::NoSelectedProvider));
    }

    #[test]
    fn selected_provider_must_be_configured() {
        let config = SpectacularConfig {
            providers: ProvidersConfig {
                selected: Some("openrouter".to_owned()),
                available: BTreeMap::new(),
            },
        };

        let error = config.validate_complete().unwrap_err();

        assert!(matches!(
            error,
            ConfigError::ProviderNotConfigured { provider } if provider == "openrouter"
        ));
    }

    #[test]
    fn selected_provider_requires_matching_api_key() {
        let mut config = SpectacularConfig::default();
        config
            .providers
            .available
            .insert("openrouter".to_owned(), ProviderConfig::default());
        config.providers.selected = Some("openrouter".to_owned());

        let error = config.validate_complete().unwrap_err();

        assert!(matches!(
            error,
            ConfigError::MissingProviderApiKey { provider } if provider == "openrouter"
        ));
    }

    #[test]
    fn completeness_requires_all_task_model_slots() {
        let mut config = complete_config();
        config
            .providers
            .available
            .get_mut("openrouter")
            .unwrap()
            .tasks
            .labeling = None;

        let error = config.validate_complete().unwrap_err();

        assert!(matches!(
            error,
            ConfigError::MissingTaskModel {
                slot: TaskModelSlot::Labeling
            }
        ));
    }

    #[test]
    fn blank_values_do_not_count_as_complete() {
        let mut config = complete_config();
        config
            .providers
            .available
            .get_mut("openrouter")
            .unwrap()
            .key = Some("   ".to_owned());

        let error = config.validate_complete().unwrap_err();

        assert!(matches!(error, ConfigError::MissingProviderApiKey { .. }));
    }

    #[test]
    fn blank_model_values_are_invalid() {
        let mut config = complete_config();
        config
            .providers
            .available
            .get_mut("openrouter")
            .unwrap()
            .tasks
            .planning = Some(TaskModelConfig::new(" ", ReasoningLevel::High));

        let error = config.validate_complete().unwrap_err();

        assert!(matches!(
            error,
            ConfigError::InvalidTaskModel {
                slot: TaskModelSlot::Planning
            }
        ));
    }

    #[test]
    fn complete_config_passes_validation() {
        let config = complete_config();

        assert!(config.validate_complete().is_ok());
        assert!(config.is_complete());
    }

    #[test]
    fn config_round_trips_as_new_json() {
        let path = temp_config_path("round-trip");
        let config = complete_config();

        write_config_to_path(&path, &config).unwrap();
        let loaded = read_config_from_path(&path).unwrap();

        assert_eq!(loaded, config);

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("\"providers\""));
        assert!(!content.contains("\"selected_provider\""));
        assert!(!content.contains("\"provider_api_keys\""));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn missing_file_returns_typed_error() {
        let path = temp_config_path("missing");
        let error = read_config_from_path(&path).unwrap_err();

        assert!(matches!(error, ConfigError::MissingConfigFile { .. }));
    }

    #[test]
    fn invalid_json_returns_typed_error() {
        let path = temp_config_path("invalid-json");
        write_text(&path, "{not json");

        let error = read_config_from_path(&path).unwrap_err();

        assert!(matches!(error, ConfigError::InvalidJson { .. }));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn config_accepts_optional_utf8_bom() {
        let path = temp_config_path("utf8-bom");
        let json = serde_json::to_string(&complete_config()).unwrap();
        write_text(&path, &format!("\u{feff}{json}"));

        let loaded = read_config_from_path(&path).unwrap();

        assert!(loaded.is_complete());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn legacy_config_migrates_on_read() {
        let path = temp_config_path("legacy");
        write_text(
            &path,
            r#"{
  "selected_provider": "openrouter",
  "provider_api_keys": {
    "openrouter": "sk-or-v1-test"
  },
  "task_models": {
    "planning": "openrouter/planning",
    "labeling": "openrouter/labeling",
    "coding": "openrouter/coding"
  }
}"#,
        );

        let loaded = read_config_from_path(&path).unwrap();
        let provider = loaded.providers.available.get("openrouter").unwrap();

        assert_eq!(loaded.providers.selected.as_deref(), Some("openrouter"));
        assert_eq!(provider.key.as_deref(), Some("sk-or-v1-test"));
        assert_eq!(
            provider.tasks.planning.as_ref().unwrap(),
            &TaskModelConfig::new("openrouter/planning", ReasoningLevel::None)
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn legacy_selected_provider_without_key_migrates_as_configured_provider() {
        let path = temp_config_path("legacy-selected-only");
        write_text(
            &path,
            r#"{
  "selected_provider": "openrouter"
}"#,
        );

        let loaded = read_config_from_path(&path).unwrap();
        let error = loaded.validate_complete().unwrap_err();

        assert!(loaded.providers.available.contains_key("openrouter"));
        assert!(matches!(
            error,
            ConfigError::MissingProviderApiKey { provider } if provider == "openrouter"
        ));

        let _ = fs::remove_file(path);
    }

    #[cfg(windows)]
    #[test]
    fn windows_config_dir_uses_appdata() {
        let appdata = PathBuf::from(r"C:\Users\example\AppData\Roaming");
        let config_dir = config_dir_with_env(|key| {
            if key == "APPDATA" {
                Some(appdata.clone().into_os_string())
            } else {
                None
            }
        })
        .unwrap();

        assert_eq!(config_dir, appdata.join(APP_CONFIG_DIR_NAME));
    }

    #[test]
    fn select_provider_updates_active_provider_when_configured() {
        let mut config = SpectacularConfig::default();
        config.set_provider_api_key("openrouter", "sk-or-v1-test");

        config.select_provider("openrouter").unwrap();

        assert_eq!(config.providers.selected.as_deref(), Some("openrouter"));
    }

    #[test]
    fn select_provider_rejects_unconfigured_provider() {
        let mut config = SpectacularConfig::default();
        let error = config.select_provider("openrouter").unwrap_err();

        assert!(matches!(
            error,
            ConfigError::ProviderNotConfigured { provider } if provider == "openrouter"
        ));
    }

    #[test]
    fn set_provider_api_key_replaces_only_selected_provider_key() {
        let mut config = SpectacularConfig::default();
        config.set_provider_api_key("openrouter", "old-openrouter-key");
        config.set_provider_api_key("openai", "existing-openai-key");

        config.set_provider_api_key("openrouter", "new-openrouter-key");

        assert_eq!(
            config
                .providers
                .available
                .get("openrouter")
                .and_then(|provider| provider.key.as_deref()),
            Some("new-openrouter-key")
        );
        assert_eq!(
            config
                .providers
                .available
                .get("openai")
                .and_then(|provider| provider.key.as_deref()),
            Some("existing-openai-key")
        );
    }

    #[test]
    fn set_provider_task_model_replaces_only_one_slot() {
        let mut config = SpectacularConfig::default();
        config.set_provider_task_model(
            "openrouter",
            TaskModelSlot::Planning,
            "openrouter/planning",
            ReasoningLevel::High,
        );

        assert_eq!(
            config
                .providers
                .available
                .get("openrouter")
                .unwrap()
                .tasks
                .planning,
            Some(TaskModelConfig::new(
                "openrouter/planning",
                ReasoningLevel::High
            ))
        );
        assert!(config
            .providers
            .available
            .get("openrouter")
            .unwrap()
            .tasks
            .labeling
            .is_none());
    }

    #[test]
    fn parsing_rejects_unknown_tasks() {
        let error = "reviewing".parse::<TaskModelSlot>().unwrap_err();

        assert!(matches!(error, ConfigParseError::InvalidTask { .. }));
    }

    #[test]
    fn parsing_rejects_unknown_reasoning() {
        let error = "hight".parse::<ReasoningLevel>().unwrap_err();

        assert!(matches!(error, ConfigParseError::InvalidReasoning { .. }));
    }

    fn complete_config() -> SpectacularConfig {
        let mut config = SpectacularConfig::default();
        config.set_provider_api_key("openrouter", "sk-or-v1-test");
        config
            .select_provider("openrouter")
            .expect("provider should be configured");
        config.set_provider_task_model(
            "openrouter",
            TaskModelSlot::Planning,
            "openrouter/planning",
            ReasoningLevel::High,
        );
        config.set_provider_task_model(
            "openrouter",
            TaskModelSlot::Labeling,
            "openrouter/labeling",
            ReasoningLevel::Low,
        );
        config.set_provider_task_model(
            "openrouter",
            TaskModelSlot::Coding,
            "openrouter/coding",
            ReasoningLevel::Medium,
        );
        config
    }

    fn temp_config_path(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        env::temp_dir()
            .join(format!("spectacular-config-test-{name}-{suffix}"))
            .join(CONFIG_FILE_NAME)
    }

    fn write_text(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }

        fs::write(path, content).unwrap();
    }
}
