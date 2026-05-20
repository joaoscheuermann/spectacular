#[tokio::main]
async fn main() -> ExitCode {
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
        Command::Chat(args) => match chat::run(debug_logger, args.tui).await {
            Ok(()) | Err(chat::ChatError::Exit) => Ok(None),
            Err(error) => Err(error.into()),
        },
        Command::Config(args) => handle_config(args).map(Some),
        Command::Plan { prompt } => handle_plan(&prompt).map(Some),
    }
}

fn handle_config(args: ConfigArgs) -> Result<String, AppError> {
    handle_config_with_io(
        args,
        spectacular_config::read_config_or_default,
        spectacular_config::read_model_cache_or_default,
        spectacular_config::backup_config,
        spectacular_config::write_config,
    )
}
