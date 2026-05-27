use super::completion::CompletionCommandSpec;
use super::context::ChatCommandContext;
use super::contract::{ChatCommand, ChatCommandResult};
use spectacular_commands::{
    Command, CommandControl, CommandError, CommandFuture, CommandInvocation, CommandRegistry,
};
use std::collections::BTreeMap;
use std::sync::Arc;

pub struct ChatCommandAdapter {
    commands: BTreeMap<&'static str, ChatCommand>,
    metadata: Arc<CommandRegistry<()>>,
    completion_specs: Vec<CompletionCommandSpec>,
}

impl ChatCommandAdapter {
    /// Registers command handlers and builds shared command metadata for prompt completion.
    pub fn new<const N: usize>(commands: [ChatCommand; N]) -> Result<Self, CommandError> {
        let mut handlers = BTreeMap::new();
        let mut metadata = CommandRegistry::new();
        let mut completion_specs = Vec::new();
        for command in commands {
            metadata.register(Command {
                name: command.name,
                usage: command.usage,
                summary: command.summary,
                execute: metadata_execute,
            })?;
            if !command.completion.is_empty() {
                completion_specs.push(CompletionCommandSpec {
                    name: command.name,
                    subcommands: command.completion,
                });
            }
            handlers.insert(command.name, command);
        }

        Ok(Self {
            commands: handlers,
            metadata: Arc::new(metadata),
            completion_specs,
        })
    }

    /// Executes a registered chat command invocation against the provided context.
    pub async fn execute(
        &self,
        context: ChatCommandContext<'_>,
        invocation: CommandInvocation,
    ) -> ChatCommandResult {
        let Some(command) = self.commands.get(invocation.name.as_str()) else {
            return ChatCommandResult::error(
                CommandError::UnknownCommand {
                    name: invocation.name,
                }
                .to_string(),
            );
        };

        (command.execute)(context, invocation.args).await
    }

    /// Returns command metadata used by the prompt editor command picker.
    pub fn metadata(&self) -> &Arc<CommandRegistry<()>> {
        &self.metadata
    }

    /// Returns subcommand and field metadata used by prompt completion.
    pub fn completion_specs(&self) -> &[CompletionCommandSpec] {
        &self.completion_specs
    }
}

/// Provides a no-op executor for metadata-only command registry entries.
fn metadata_execute<'a>(_context: &'a mut (), _args: Vec<String>) -> CommandFuture<'a> {
    Box::pin(async { Ok(CommandControl::Continue) })
}
