use spectacular_config::{ConfigError, SpectacularConfig};
use std::error::Error;
use std::fmt::{self, Display};

const PLACEHOLDER_PLAN_OUTPUT: &str = "Hello World";

/// Runs the initial planning route after validating prompt and configuration.
pub fn run(
    prompt: &str,
    load_config: impl FnOnce() -> Result<SpectacularConfig, ConfigError>,
) -> Result<&'static str, PlanError> {
    validate_prompt(prompt)?;

    let config = load_config().map_err(PlanError::Config)?;
    config.validate_complete().map_err(PlanError::Config)?;

    Ok(PLACEHOLDER_PLAN_OUTPUT)
}

/// Validates that the plan prompt contains non-whitespace text.
pub fn validate_prompt(prompt: &str) -> Result<(), PlanError> {
    if !prompt.trim().is_empty() {
        return Ok(());
    }

    Err(PlanError::EmptyPrompt)
}

#[derive(Debug)]
pub enum PlanError {
    EmptyPrompt,
    Config(ConfigError),
}

impl Display for PlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlanError::EmptyPrompt => formatter.write_str("a non-empty prompt is required"),
            PlanError::Config(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for PlanError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            PlanError::EmptyPrompt => None,
            PlanError::Config(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spectacular_config::{
        ProviderConfig, ProvidersConfig, ReasoningLevel, TaskModelConfig, TaskModelSlot, TaskModels,
    };
    use std::collections::BTreeMap;

    #[test]
    fn rejects_whitespace_only_prompt_before_loading_config() {
        let output = run("   ", || panic!("config should not be loaded"));

        assert!(matches!(output, Err(PlanError::EmptyPrompt)));
    }

    #[test]
    fn returns_config_errors_for_missing_config() {
        let output = run("Create a login flow", || {
            Err(ConfigError::MissingConfigFile {
                path: "config.json".into(),
            })
        });

        assert!(matches!(
            output,
            Err(PlanError::Config(ConfigError::MissingConfigFile { .. }))
        ));
    }

    #[test]
    fn rejects_incomplete_config() {
        let output = run("Create a login flow", || {
            let mut config = complete_config();
            config
                .providers
                .available
                .get_mut("openrouter")
                .unwrap()
                .tasks
                .coding = None;
            Ok(config)
        });

        assert!(matches!(
            output,
            Err(PlanError::Config(ConfigError::MissingTaskModel {
                slot: TaskModelSlot::Coding
            }))
        ));
    }

    #[test]
    fn complete_config_prints_placeholder_output() {
        let output = run("Create a login flow", || Ok(complete_config())).unwrap();

        assert_eq!(output, "Hello World");
    }

    fn complete_config() -> SpectacularConfig {
        let mut available = BTreeMap::new();
        available.insert(
            "openrouter".to_owned(),
            ProviderConfig {
                key: Some("sk-or-v1-test".to_owned()),
                tasks: TaskModels {
                    planning: Some(TaskModelConfig::new(
                        "openrouter/planning",
                        ReasoningLevel::None,
                    )),
                    labeling: Some(TaskModelConfig::new(
                        "openrouter/labeling",
                        ReasoningLevel::None,
                    )),
                    coding: Some(TaskModelConfig::new(
                        "openrouter/coding",
                        ReasoningLevel::None,
                    )),
                },
            },
        );

        SpectacularConfig {
            providers: ProvidersConfig {
                selected: Some("openrouter".to_owned()),
                available,
            },
        }
    }
}
