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
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/unit/plan.rs"));
}
