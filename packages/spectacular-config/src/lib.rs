use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::ffi::OsString;
use std::fmt::{self, Display};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const APP_CONFIG_DIR_NAME: &str = "spectacular";
const CONFIG_FILE_NAME: &str = "config.json";

/// Task model identifiers required before commands can use LLM-backed behavior.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct TaskModels {
    pub planning: Option<String>,
    pub labeling: Option<String>,
    pub coding: Option<String>,
}

/// Persisted Spectacular CLI configuration.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct SpectacularConfig {
    pub selected_provider: Option<String>,
    pub provider_api_keys: BTreeMap<String, String>,
    pub task_models: TaskModels,
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
    pub fn select_provider(&mut self, provider_id: impl Into<String>) {
        self.selected_provider = Some(provider_id.into());
    }

    /// Replaces the stored API key for a provider without touching other provider keys.
    pub fn set_provider_api_key(
        &mut self,
        provider_id: impl Into<String>,
        api_key: impl Into<String>,
    ) {
        self.provider_api_keys
            .insert(provider_id.into(), api_key.into());
    }

    /// Saves all task model assignments together.
    pub fn set_task_models(
        &mut self,
        planning: impl Into<String>,
        labeling: impl Into<String>,
        coding: impl Into<String>,
    ) {
        self.task_models = TaskModels {
            planning: Some(planning.into()),
            labeling: Some(labeling.into()),
            coding: Some(coding.into()),
        };
    }

    /// Validates that all required configuration fields are present and non-empty.
    pub fn validate_complete(&self) -> Result<(), ConfigError> {
        let provider = required_text(self.selected_provider.as_deref())
            .ok_or(ConfigError::NoSelectedProvider)?;

        let api_key = self
            .provider_api_keys
            .get(provider)
            .and_then(|value| required_text(Some(value.as_str())));

        if api_key.is_none() {
            return Err(ConfigError::MissingProviderApiKey {
                provider: provider.to_owned(),
            });
        }

        require_task_model(
            TaskModelSlot::Planning,
            self.task_models.planning.as_deref(),
        )?;
        require_task_model(
            TaskModelSlot::Labeling,
            self.task_models.labeling.as_deref(),
        )?;
        require_task_model(TaskModelSlot::Coding, self.task_models.coding.as_deref())?;

        Ok(())
    }

    /// Returns true when all required setup values are present and non-empty.
    pub fn is_complete(&self) -> bool {
        self.validate_complete().is_ok()
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
    MissingProviderApiKey {
        provider: String,
    },
    MissingTaskModel {
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

impl Display for TaskModelSlot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

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
            ConfigError::MissingProviderApiKey { provider } => {
                write!(formatter, "provider `{provider}` does not have an API key")
            }
            ConfigError::MissingTaskModel { slot } => {
                write!(formatter, "missing `{slot}` model assignment")
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
            | ConfigError::MissingProviderApiKey { .. }
            | ConfigError::MissingTaskModel { .. } => None,
        }
    }
}

fn require_task_model(slot: TaskModelSlot, value: Option<&str>) -> Result<(), ConfigError> {
    if required_text(value).is_some() {
        return Ok(());
    }

    Err(ConfigError::MissingTaskModel { slot })
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
    fn selected_provider_requires_matching_api_key() {
        let config = SpectacularConfig {
            selected_provider: Some("openrouter".to_owned()),
            ..SpectacularConfig::default()
        };

        let error = config.validate_complete().unwrap_err();

        assert!(matches!(
            error,
            ConfigError::MissingProviderApiKey { provider } if provider == "openrouter"
        ));
    }

    #[test]
    fn completeness_requires_all_task_model_slots() {
        let mut config = complete_config();
        config.task_models.labeling = None;

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
            .provider_api_keys
            .insert("openrouter".to_owned(), "   ".to_owned());

        let error = config.validate_complete().unwrap_err();

        assert!(matches!(error, ConfigError::MissingProviderApiKey { .. }));
    }

    #[test]
    fn complete_config_passes_validation() {
        let config = complete_config();

        assert!(config.validate_complete().is_ok());
        assert!(config.is_complete());
    }

    #[test]
    fn config_round_trips_as_json() {
        let path = temp_config_path("round-trip");
        let config = complete_config();

        write_config_to_path(&path, &config).unwrap();
        let loaded = read_config_from_path(&path).unwrap();

        assert_eq!(loaded, config);

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

    fn complete_config() -> SpectacularConfig {
        let mut provider_api_keys = BTreeMap::new();
        provider_api_keys.insert("openrouter".to_owned(), "sk-or-v1-test".to_owned());

        SpectacularConfig {
            selected_provider: Some("openrouter".to_owned()),
            provider_api_keys,
            task_models: TaskModels {
                planning: Some("openrouter/planning".to_owned()),
                labeling: Some("openrouter/labeling".to_owned()),
                coding: Some("openrouter/coding".to_owned()),
            },
        }
    }

    #[test]
    fn select_provider_updates_active_provider() {
        let mut config = SpectacularConfig::default();

        config.select_provider("openrouter");

        assert_eq!(config.selected_provider.as_deref(), Some("openrouter"));
    }

    #[test]
    fn set_provider_api_key_replaces_only_selected_provider_key() {
        let mut config = SpectacularConfig::default();
        config.set_provider_api_key("openrouter", "old-openrouter-key");
        config.set_provider_api_key("openai", "existing-openai-key");

        config.set_provider_api_key("openrouter", "new-openrouter-key");

        assert_eq!(
            config
                .provider_api_keys
                .get("openrouter")
                .map(String::as_str),
            Some("new-openrouter-key")
        );
        assert_eq!(
            config.provider_api_keys.get("openai").map(String::as_str),
            Some("existing-openai-key")
        );
    }

    #[test]
    fn set_task_models_replaces_assignments_together() {
        let mut config = SpectacularConfig::default();

        config.set_task_models(
            "openrouter/planning",
            "openrouter/labeling",
            "openrouter/coding",
        );

        assert_eq!(
            config.task_models,
            TaskModels {
                planning: Some("openrouter/planning".to_owned()),
                labeling: Some("openrouter/labeling".to_owned()),
                coding: Some("openrouter/coding".to_owned()),
            }
        );
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
