use super::{
    chat,
    cli_types::{Cli, Command, ConfigArgs},
    config_ops::{handle_config_with_io, ConfigIo},
    lifecycle::handle_lifecycle_command,
    output::user_facing_error,
};
use ::config::ConfigError;
use ::llms::{LlmDebugLogger, ProviderError};
use clap::Parser;
use std::future::Future;
use std::pin::Pin;
use std::process::ExitCode;

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

#[derive(Debug)]
pub(super) enum AppError {
    Chat(chat::ChatError),
    Config(ConfigError),
    DebugLog { source: std::io::Error },
    InvalidConfigCommand(String),
    InvalidLifecycleCommand(String),
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

    match dispatch(cli).await {
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

async fn dispatch(cli: Cli) -> Result<Option<String>, AppError> {
    dispatch_with_dependencies(
        cli,
        DispatchDependencies::new_async(
            LlmDebugLogger::create_for_current_exe,
            chat::run,
            handle_config,
            handle_lifecycle,
        ),
    )
    .await
}

pub(super) struct DispatchDependencies<'a, DebugLogger> {
    create_debug_logger: Box<dyn FnOnce() -> Result<DebugLogger, std::io::Error> + 'a>,
    run_chat: Box<dyn FnOnce(DebugLogger) -> BoxFuture<'a, Result<String, chat::ChatError>> + 'a>,
    run_config: Box<dyn FnOnce(ConfigArgs) -> Result<String, AppError> + 'a>,
    run_lifecycle: Box<dyn FnOnce(Command) -> Result<Option<String>, AppError> + 'a>,
}

impl<'a, DebugLogger: 'a> DispatchDependencies<'a, DebugLogger> {
    #[cfg(test)]
    pub(super) fn new<CreateDebugLogger, RunChat, RunConfig, RunLifecycle>(
        create_debug_logger: CreateDebugLogger,
        run_chat: RunChat,
        run_config: RunConfig,
        run_lifecycle: RunLifecycle,
    ) -> Self
    where
        CreateDebugLogger: FnOnce() -> Result<DebugLogger, std::io::Error> + 'a,
        RunChat: FnOnce(DebugLogger) -> Result<String, chat::ChatError> + 'a,
        RunConfig: FnOnce(ConfigArgs) -> Result<String, AppError> + 'a,
        RunLifecycle: FnOnce(Command) -> Result<Option<String>, AppError> + 'a,
    {
        Self::new_async(
            create_debug_logger,
            |debug_logger| async { run_chat(debug_logger) },
            run_config,
            run_lifecycle,
        )
    }

    fn new_async<CreateDebugLogger, RunChat, ChatFuture, RunConfig, RunLifecycle>(
        create_debug_logger: CreateDebugLogger,
        run_chat: RunChat,
        run_config: RunConfig,
        run_lifecycle: RunLifecycle,
    ) -> Self
    where
        CreateDebugLogger: FnOnce() -> Result<DebugLogger, std::io::Error> + 'a,
        RunChat: FnOnce(DebugLogger) -> ChatFuture + 'a,
        ChatFuture: Future<Output = Result<String, chat::ChatError>> + 'a,
        RunConfig: FnOnce(ConfigArgs) -> Result<String, AppError> + 'a,
        RunLifecycle: FnOnce(Command) -> Result<Option<String>, AppError> + 'a,
    {
        Self {
            create_debug_logger: Box::new(create_debug_logger),
            run_chat: Box::new(|debug_logger| Box::pin(run_chat(debug_logger))),
            run_config: Box::new(run_config),
            run_lifecycle: Box::new(run_lifecycle),
        }
    }
}

pub(super) async fn dispatch_with_dependencies<DebugLogger>(
    cli: Cli,
    dependencies: DispatchDependencies<'_, DebugLogger>,
) -> Result<Option<String>, AppError> {
    match cli.command {
        Some(Command::Config(args)) => (dependencies.run_config)(args).map(Some),
        Some(command) => (dependencies.run_lifecycle)(command),
        None => {
            let debug_logger = (dependencies.create_debug_logger)()
                .map_err(|source| AppError::DebugLog { source })?;
            match (dependencies.run_chat)(debug_logger).await {
                Ok(closed_session_id) => Ok(Some(closed_session_message(&closed_session_id))),
                Err(chat::ChatError::Exit) => Ok(None),
                Err(error) => Err(error.into()),
            }
        }
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

fn handle_lifecycle(command: Command) -> Result<Option<String>, AppError> {
    handle_lifecycle_command(command).map(Some).ok_or_else(|| {
        AppError::InvalidLifecycleCommand("Unsupported lifecycle command.".to_owned())
    })
}

#[cfg(test)]
mod tests {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/unit/entry.rs"));
}
