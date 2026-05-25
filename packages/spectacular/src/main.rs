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

use std::process::ExitCode;

#[path = "main/cli_types.rs"]
mod cli_types;
#[path = "main/config_ops.rs"]
mod config_ops;
#[path = "main/entry.rs"]
mod entry;
#[path = "main/output.rs"]
mod output;
#[path = "main/plan_errors.rs"]
mod plan_errors;

#[cfg(test)]
use clap::Parser;
#[cfg(test)]
use cli_types::{
    Cli, ConfigArgs, ConfigCommand, ConfigModelCommand, ConfigOperation, ConfigProviderCommand,
    ConfigTaskCommand,
};
#[cfg(test)]
use config_ops::{config_operation, handle_config_with_io, ConfigIo};
#[cfg(test)]
use plan_errors::{handle_plan_with_loader, user_facing_error, AppError};
#[cfg(test)]
use spectacular_config::{ConfigError, ReasoningLevel, SpectacularConfig, TaskModelSlot};
#[cfg(test)]
use spectacular_plan::PlanError;

fn main() -> ExitCode {
    entry::run()
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/main_cli.rs"
    ));
}
