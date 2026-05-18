use crate::chat::commands::{
    ChatCommandAdapter, ChatCommandContext, ChatCommandControl, ChatCommandResult,
};
use crate::chat::model::{ChatModel, ChatPromptFooterModel, ChatRunRequestModel};
use crate::chat::prompt::{PromptCompletionCatalog, PromptEditor};
use crate::chat::renderer::Renderer;
use crate::chat::runner::{ChatRunnerService, ChatTurnRunner};
use crate::chat::ChatError;
use spectacular_agent::ToolStorage;
use spectacular_commands::{parse_line, CommandControl, CommandInvocation, ParseOutcome};
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;

pub struct ChatController<R = ChatRunnerService> {
    model: ChatModel,
    commands: ChatCommandAdapter,
    renderer: Renderer,
    tools: ToolStorage,
    workspace_root: PathBuf,
    runner: R,
}

impl ChatController<ChatRunnerService> {
    /// Creates a chat controller wired to the default chat runner service.
    pub fn new(
        model: ChatModel,
        commands: ChatCommandAdapter,
        renderer: Renderer,
        tools: ToolStorage,
        workspace_root: PathBuf,
    ) -> Self {
        Self::with_runner(
            model,
            commands,
            renderer,
            tools,
            workspace_root,
            ChatRunnerService,
        )
    }
}

impl<R> ChatController<R>
where
    R: ChatTurnRunner,
{
    /// Creates a chat controller with an injected runner for tests or alternate execution.
    pub fn with_runner(
        model: ChatModel,
        commands: ChatCommandAdapter,
        renderer: Renderer,
        tools: ToolStorage,
        workspace_root: PathBuf,
        runner: R,
    ) -> Self {
        Self {
            model,
            commands,
            renderer,
            tools,
            workspace_root,
            runner,
        }
    }

    /// Executes a parsed slash command and maps command control into REPL control.
    pub async fn dispatch_command(
        &mut self,
        invocation: CommandInvocation,
    ) -> Result<CommandControl, ChatError> {
        let mut command_control = ChatCommandControl::default();
        let prompt_footer = self.prompt_footer();
        let context = ChatCommandContext::new_with_footer(
            &mut self.model,
            &self.renderer,
            &self.tools,
            &self.runner,
            &mut command_control,
            Some(prompt_footer),
        );
        let result = self.commands.execute(context, invocation).await;
        if let ChatCommandResult::Error(message) = result {
            self.renderer.error(&message);
        }

        if command_control.exit_requested() {
            return Ok(CommandControl::Exit);
        }

        Ok(CommandControl::Continue)
    }

    /// Sends a user prompt through the active runner when runtime configuration is complete.
    pub async fn dispatch_prompt(&mut self, prompt: String) -> Result<(), ChatError> {
        if !self.model.runtime().is_ready() {
            self.renderer.error(
                "configuration is incomplete; use /provider, /model, and /task commands before sending prompts",
            );
            return Ok(());
        }

        let request = ChatRunRequestModel {
            prompt,
            prompt_event_id: None,
            render_user_prompt: true,
            retry_existing_prompt: false,
            runtime: self.model.runtime().clone(),
        };
        self.runner
            .run(&mut self.model, &self.renderer, &self.tools, request)
            .await
    }

    /// Parses a REPL input line and dispatches it as a command or user prompt.
    pub async fn handle_line(&mut self, line: String) -> Result<CommandControl, ChatError> {
        let line = line.trim_end_matches(['\r', '\n']).to_owned();
        if line.trim().is_empty() {
            return Ok(CommandControl::Continue);
        }

        let parsed = match parse_line(&line) {
            Ok(parsed) => parsed,
            Err(error) => {
                self.renderer.command_error(&error);
                return Ok(CommandControl::Continue);
            }
        };

        match parsed {
            ParseOutcome::NotCommand => {
                self.dispatch_prompt(line).await?;
                Ok(CommandControl::Continue)
            }
            ParseOutcome::Command(invocation) => self.dispatch_command(invocation).await,
        }
    }

    /// Runs the interactive chat loop until a command or EOF exits it.
    pub async fn run_loop(&mut self) -> Result<(), ChatError> {
        loop {
            let line = self.read_prompt_line()?;
            if matches!(self.handle_line(line).await?, CommandControl::Exit) {
                return Ok(());
            }
        }
    }

    /// Reads one prompt line using the rich editor for terminals and stdin otherwise.
    fn read_prompt_line(&self) -> Result<String, ChatError> {
        if io::stdin().is_terminal() && io::stdout().is_terminal() {
            let completions =
                PromptCompletionCatalog::new(self.commands.completion_specs(), &self.model);
            return PromptEditor::new(&self.renderer, self.commands.metadata(), &completions)
                .with_footer(self.prompt_footer())
                .read_line();
        }

        self.renderer.prompt();
        io::stdout().flush().map_err(ChatError::Io)?;
        let mut line = String::new();
        let read = io::stdin().read_line(&mut line).map_err(ChatError::Io)?;
        if read == 0 {
            return Err(ChatError::Exit);
        }

        Ok(line)
    }

    /// Builds prompt footer data from the controller-owned workspace root and active runtime.
    fn prompt_footer(&self) -> ChatPromptFooterModel {
        ChatPromptFooterModel::from_runtime_and_usage(
            &self.workspace_root,
            self.model.runtime(),
            self.model.context_token_usage(),
        )
    }
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/controller.rs"
    ));
}
