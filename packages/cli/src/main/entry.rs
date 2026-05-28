use super::{
    chat,
    cli_types::{Cli, Command, ConfigArgs},
    config_ops::{handle_config_with_io, ConfigIo},
    output::user_facing_error,
};
use ::config::ConfigError;
use ::llms::{LlmDebugLogger, ProviderError};
use clap::Parser;
use std::process::ExitCode;

#[derive(Debug)]
pub(super) enum AppError {
    Chat(chat::ChatError),
    Config(ConfigError),
    DebugLog { source: std::io::Error },
    InvalidConfigCommand(String),
    Provider { source: Box<ProviderError> },
}

impl From<ConfigError> for AppError {
    fn from(error: ConfigError) -> Self {
        Self::Config(error)
    }
}

impl From<chat::ChatError> for AppError {
    fn from(error: chat::ChatError) -> Self {
        Self::Chat(error)
    }
}

impl From<commands::CommandError> for AppError {
    fn from(error: commands::CommandError) -> Self {
        Self::InvalidConfigCommand(error.to_string())
    }
}

impl From<ProviderError> for AppError {
    fn from(source: ProviderError) -> Self {
        Self::Provider {
            source: Box::new(source),
        }
    }
}

#[tokio::main]
pub(super) async fn run() -> ExitCode {
    let cli = Cli::parse();
    let debug_logger = match LlmDebugLogger::create_for_current_exe() {
        Ok(logger) => logger,
        Err(error) => {
            eprintln!(
                "{}",
                user_facing_error(&AppError::DebugLog { source: error })
            );
            return ExitCode::FAILURE;
        }
    };

    match handle(cli, debug_logger).await {
        Ok(Some(output)) => {
            if !output.is_empty() {
                println!("{output}");
            }
            ExitCode::SUCCESS
        }
        Ok(None) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{}", user_facing_error(&error));
            ExitCode::FAILURE
        }
    }
}

async fn handle(cli: Cli, debug_logger: LlmDebugLogger) -> Result<Option<String>, AppError> {
    match cli.command {
        None => match chat::run(debug_logger).await {
            Ok(closed_session_id) => Ok(Some(closed_session_message(&closed_session_id))),
            Err(chat::ChatError::Exit) => Ok(None),
            Err(error) => Err(error.into()),
        },
        Some(Command::Config(args)) => handle_config(args).map(Some),
    }
}

fn closed_session_message(session_id: &str) -> String {
    format!("Closed session: {session_id}")
}

fn handle_config(args: ConfigArgs) -> Result<String, AppError> {
    handle_config_with_io(
        args,
        ConfigIo::new(
            config::read_config_or_default,
            config::read_model_cache_or_default,
            config::backup_config,
            config::write_config,
        ),
    )
}
