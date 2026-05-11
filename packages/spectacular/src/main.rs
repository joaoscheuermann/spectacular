mod chat;
mod config_fields;
pub(crate) mod terminal_style;
#[cfg(test)]
mod terminal_style_tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/terminal_style.rs"
    ));
}

use anstyle::Style;
use clap::{Args, Parser, Subcommand};
use spectacular_commands::NamedArgs;
use spectacular_config::{
    mask_api_key, CachedModelMetadata, ConfigError, ModelCache, ReasoningLevel, SpectacularConfig,
    TaskModelSlot,
};
use spectacular_llms::{LlmDebugLogger, ProviderError, ProviderMetadata};
use spectacular_plan::PlanError;
use std::process::ExitCode;

include!("main/cli_types.rs");
include!("main/entry.rs");
include!("main/config_ops.rs");
include!("main/output.rs");
include!("main/plan_errors.rs");

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/main_cli.rs"
    ));
}
