use spectacular_commands::{CommandError, NamedArgs};
use spectacular_config::{ReasoningLevel, TaskModelSlot};
use std::str::FromStr;

/// Parses shell-style `key=value` command fields and rejects names outside the allowed set.
pub(crate) fn named_args(fields: &[String], allowed: &[&str]) -> Result<NamedArgs, CommandError> {
    let args = NamedArgs::parse(fields)?;
    args.reject_unknown(allowed)?;
    Ok(args)
}

/// Parses a persisted reasoning-level value using the config crate's canonical grammar.
pub(crate) fn parse_reasoning(
    value: &str,
) -> Result<ReasoningLevel, spectacular_config::ConfigParseError> {
    ReasoningLevel::from_str(value)
}

/// Parses a task model slot and normalizes parse failures into command-facing errors.
pub(crate) fn parse_task(value: &str) -> Result<TaskModelSlot, CommandError> {
    TaskModelSlot::from_str(value).map_err(|error| CommandError::message(error.to_string()))
}

/// Returns whether a provider type is registered and enabled for user-facing configuration.
pub(crate) fn provider_type_enabled(provider_type: &str) -> bool {
    spectacular_llms::provider_by_id(provider_type).is_some_and(|provider| provider.is_enabled())
}
