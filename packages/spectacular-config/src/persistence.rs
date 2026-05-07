/// Handles the config dir persistence helper for Spectacular config files.
pub fn config_dir() -> Result<PathBuf, ConfigError> {
    config_dir_with_env(|key| env::var_os(key))
}
/// Returns the platform-specific `config.json` path.
pub fn config_path() -> Result<PathBuf, ConfigError> {
    Ok(config_dir()?.join(CONFIG_FILE_NAME))
}

/// Returns the platform-specific `model-cache.json` path.
pub fn model_cache_path() -> Result<PathBuf, ConfigError> {
    Ok(config_dir()?.join(MODEL_CACHE_FILE_NAME))
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

/// Writes `config.json` through a temporary file and replace step.
pub fn write_config(config: &SpectacularConfig) -> Result<(), ConfigError> {
    write_config_to_path(&config_path()?, config)
}

/// Creates a timestamped backup beside the current config file.
pub fn backup_config() -> Result<Option<PathBuf>, ConfigError> {
    backup_config_file(&config_path()?)
}

/// Reads `model-cache.json`, returning an empty cache when absent.
pub fn read_model_cache_or_default() -> Result<ModelCache, ConfigError> {
    let path = model_cache_path()?;
    if !path.exists() {
        return Ok(ModelCache::default());
    }

    read_model_cache_from_path(&path)
}

/// Writes `model-cache.json` through the same temp-file path as config writes.
pub fn write_model_cache(cache: &ModelCache) -> Result<(), ConfigError> {
    write_model_cache_to_path(&model_cache_path()?, cache)
}

/// Formats config as pretty JSON using the persisted schema.
pub fn to_pretty_json(config: &SpectacularConfig) -> Result<String, ConfigError> {
    serde_json::to_string_pretty(config).map_err(|source| ConfigError::SerializeFailed {
        path: config_path().unwrap_or_else(|_| PathBuf::from(CONFIG_FILE_NAME)),
        source,
    })
}

/// Handles the read config from path persistence helper for Spectacular config files.
pub fn read_config_from_path(path: &Path) -> Result<SpectacularConfig, ConfigError> {
    read_json_from_path(path)
}

/// Handles the write config to path persistence helper for Spectacular config files.
pub fn write_config_to_path(path: &Path, config: &SpectacularConfig) -> Result<(), ConfigError> {
    write_json_to_path(path, config)
}

/// Handles the read model cache from path persistence helper for Spectacular config files.
pub fn read_model_cache_from_path(path: &Path) -> Result<ModelCache, ConfigError> {
    read_json_from_path(path)
}

/// Handles the write model cache to path persistence helper for Spectacular config files.
pub fn write_model_cache_to_path(path: &Path, cache: &ModelCache) -> Result<(), ConfigError> {
    write_json_to_path(path, cache)
}

/// Creates a timestamped backup beside `path`, returning `Ok(None)` when absent.
pub fn backup_config_file(path: &Path) -> Result<Option<PathBuf>, ConfigError> {
    if !path.exists() {
        return Ok(None);
    }

    let backup = backup_path_for(path);
    fs::copy(path, &backup).map_err(|source| ConfigError::WriteFailed {
        path: backup.clone(),
        source,
    })?;

    Ok(Some(backup))
}

/// Handles the composite model key persistence helper for Spectacular config files.
pub fn composite_model_key(provider: &str, model: &str) -> String {
    format!("{provider}:{model}")
}

/// Handles the mask api key persistence helper for Spectacular config files.
pub fn mask_api_key(api_key: &str) -> String {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return "not configured".to_owned();
    }

    let chars = api_key.chars().collect::<Vec<_>>();
    if chars.len() <= 8 {
        return "*".repeat(chars.len().max(4));
    }

    let start = chars.iter().take(4).collect::<String>();
    let end = chars
        .iter()
        .skip(chars.len().saturating_sub(4))
        .collect::<String>();
    format!("{start}...{end}")
}

/// Handles the read json from path persistence helper for Spectacular config files.
fn read_json_from_path<T>(path: &Path) -> Result<T, ConfigError>
where
    T: for<'de> Deserialize<'de>,
{
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

    serde_json::from_str(json).map_err(|source| {
        if source.to_string().contains(SCHEMA_CHANGED_MESSAGE) {
            ConfigError::SchemaChanged
        } else {
            ConfigError::InvalidJson {
                path: path.to_path_buf(),
                source,
            }
        }
    })
}

/// Handles the write json to path persistence helper for Spectacular config files.
fn write_json_to_path<T>(path: &Path, value: &T) -> Result<(), ConfigError>
where
    T: Serialize,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| ConfigError::WriteFailed {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let content =
        serde_json::to_string_pretty(value).map_err(|source| ConfigError::SerializeFailed {
            path: path.to_path_buf(),
            source,
        })?;
    let temp = temp_path_for(path);
    fs::write(&temp, content).map_err(|source| ConfigError::WriteFailed {
        path: temp.clone(),
        source,
    })?;

    match fs::rename(&temp, path) {
        Ok(()) => Ok(()),
        Err(first_error) if path.exists() => {
            fs::remove_file(path).map_err(|source| ConfigError::WriteFailed {
                path: path.to_path_buf(),
                source,
            })?;
            fs::rename(&temp, path).map_err(|source| ConfigError::WriteFailed {
                path: path.to_path_buf(),
                source,
            })?;
            drop(first_error);
            Ok(())
        }
        Err(source) => Err(ConfigError::WriteFailed {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Handles the temp path for persistence helper for Spectacular config files.
fn temp_path_for(path: &Path) -> PathBuf {
    let suffix = timestamp();
    path.with_extension(format!("tmp-{suffix}"))
}

/// Handles the backup path for persistence helper for Spectacular config files.
fn backup_path_for(path: &Path) -> PathBuf {
    let suffix = timestamp();
    path.with_extension(format!("{suffix}.{BACKUP_EXTENSION}"))
}

/// Handles the timestamp persistence helper for Spectacular config files.
fn timestamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

/// Handles the require text persistence helper for Spectacular config files.
fn require_text(field: &'static str, value: String) -> Result<String, ConfigError> {
    if !value.trim().is_empty() {
        return Ok(value);
    }

    Err(ConfigError::EmptyValue { field })
}

/// Handles the required text persistence helper for Spectacular config files.
fn required_text(value: Option<&str>) -> Option<&str> {
    value.filter(|text| !text.trim().is_empty())
}

/// Handles the config dir with env persistence helper for Spectacular config files.
fn config_dir_with_env(get_env: impl Fn(&str) -> Option<OsString>) -> Result<PathBuf, ConfigError> {
    platform_config_base(get_env)
        .map(|base| base.join(APP_CONFIG_DIR_NAME))
        .ok_or(ConfigError::ConfigDirUnavailable)
}

#[cfg(windows)]
/// Handles the platform config base persistence helper for Spectacular config files.
fn platform_config_base(get_env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    get_env("APPDATA").map(PathBuf::from)
}

#[cfg(target_os = "macos")]
/// Handles the platform config base persistence helper for Spectacular config files.
fn platform_config_base(get_env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    get_env("HOME")
        .map(PathBuf::from)
        .map(|home| home.join("Library").join("Application Support"))
}

#[cfg(all(unix, not(target_os = "macos")))]
/// Handles the platform config base persistence helper for Spectacular config files.
fn platform_config_base(get_env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    get_env("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| {
        get_env("HOME")
            .map(PathBuf::from)
            .map(|home| home.join(".config"))
    })
}

/// Handles the looks like legacy schema persistence helper for Spectacular config files.
fn looks_like_legacy_schema(value: &serde_json::Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };

    if object.contains_key("selected_provider")
        || object.contains_key("provider_api_keys")
        || object.contains_key("task_models")
    {
        return true;
    }

    object
        .get("providers")
        .and_then(serde_json::Value::as_object)
        .is_some_and(|providers| {
            providers.contains_key("selected") || providers.contains_key("available")
        })
}
