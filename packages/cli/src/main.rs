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
#[path = "main/lifecycle.rs"]
mod lifecycle;
#[path = "main/output.rs"]
mod output;

#[cfg(test)]
use ::config::{ConfigError, DoricConfig, ReasoningLevel, TaskModelSlot};
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
use entry::AppError;
#[cfg(test)]
use output::user_facing_error;

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
