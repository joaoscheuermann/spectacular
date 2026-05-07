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
    SchemaChanged,
    EmptyValue {
        field: &'static str,
    },
    ProviderAlreadyExists {
        provider: String,
    },
    ProviderNotConfigured {
        provider: String,
    },
    InvalidProviderType {
        provider: String,
    },
    MissingProviderApiKey {
        provider: String,
    },
    ModelAlreadyExists {
        model: String,
    },
    ModelNotConfigured {
        model: String,
    },
    ModelProviderNotConfigured {
        model: String,
        provider: String,
    },
    MissingTaskModel {
        slot: TaskModelSlot,
    },
    InvalidTaskModelReference {
        slot: TaskModelSlot,
        model: String,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskModelSlot {
    General,
    Coding,
    Labeling,
}

impl TaskModelSlot {
    pub const ALL: [TaskModelSlot; 3] = [
        TaskModelSlot::General,
        TaskModelSlot::Coding,
        TaskModelSlot::Labeling,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            TaskModelSlot::General => "general",
            TaskModelSlot::Coding => "coding",
            TaskModelSlot::Labeling => "labeling",
        }
    }
}

impl FromStr for TaskModelSlot {
    type Err = ConfigParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "general" => Ok(TaskModelSlot::General),
            "coding" => Ok(TaskModelSlot::Coding),
            "labeling" => Ok(TaskModelSlot::Labeling),
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
                "invalid reasoning `{value}`; expected one of: none, minimal, low, medium, high, xhigh"
            ),
            ConfigParseError::InvalidTask { value } => write!(
                formatter,
                "invalid task `{value}`; expected one of: general, coding, labeling"
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
            ConfigError::InvalidJson { path, source } => {
                if source.to_string().contains(SCHEMA_CHANGED_MESSAGE) {
                    return formatter.write_str(SCHEMA_CHANGED_MESSAGE);
                }
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
            ConfigError::SchemaChanged => formatter.write_str(SCHEMA_CHANGED_MESSAGE),
            ConfigError::EmptyValue { field } => write!(formatter, "`{field}` cannot be empty"),
            ConfigError::ProviderAlreadyExists { provider } => {
                write!(formatter, "provider `{provider}` already exists")
            }
            ConfigError::ProviderNotConfigured { provider } => {
                write!(formatter, "provider `{provider}` is not configured")
            }
            ConfigError::InvalidProviderType { provider } => {
                write!(
                    formatter,
                    "provider `{provider}` does not have a provider type"
                )
            }
            ConfigError::MissingProviderApiKey { provider } => {
                write!(formatter, "provider `{provider}` does not have an API key")
            }
            ConfigError::ModelAlreadyExists { model } => {
                write!(formatter, "model `{model}` already exists")
            }
            ConfigError::ModelNotConfigured { model } => {
                write!(formatter, "model `{model}` is not configured")
            }
            ConfigError::ModelProviderNotConfigured { model, provider } => write!(
                formatter,
                "model `{model}` references missing provider `{provider}`"
            ),
            ConfigError::MissingTaskModel { slot } => {
                write!(formatter, "missing `{slot}` model assignment")
            }
            ConfigError::InvalidTaskModelReference { slot, model } => write!(
                formatter,
                "`{slot}` task references missing model `{model}`"
            ),
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
            | ConfigError::SchemaChanged
            | ConfigError::EmptyValue { .. }
            | ConfigError::ProviderAlreadyExists { .. }
            | ConfigError::ProviderNotConfigured { .. }
            | ConfigError::InvalidProviderType { .. }
            | ConfigError::MissingProviderApiKey { .. }
            | ConfigError::ModelAlreadyExists { .. }
            | ConfigError::ModelNotConfigured { .. }
            | ConfigError::ModelProviderNotConfigured { .. }
            | ConfigError::MissingTaskModel { .. }
            | ConfigError::InvalidTaskModelReference { .. } => None,
        }
    }
}
