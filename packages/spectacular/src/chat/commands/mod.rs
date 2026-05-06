pub mod config;
pub mod git;
pub mod runtime;
pub mod session;

use crate::chat::model::{ChatModel, ChatRunRequestModel, HistoryTableModel};
use crate::chat::renderer::Renderer;
use crate::chat::runner::ChatTurnRunner;
use crate::chat::session::ChatRecord;
use crate::chat::ChatError;
use spectacular_agent::{AgentEvent, ToolStorage};
use spectacular_commands::{
    Command, CommandControl, CommandError, CommandFuture, CommandInvocation, CommandRegistry,
};
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub type ChatCommandFuture<'a> = Pin<Box<dyn Future<Output = ChatCommandResult> + 'a>>;

pub type ChatCommandHandler =
    for<'a> fn(ChatCommandContext<'a>, Vec<String>) -> ChatCommandFuture<'a>;

#[derive(Debug, Eq, PartialEq)]
pub enum ChatCommandResult {
    Success,
    Error(String),
}

impl ChatCommandResult {
    pub fn success() -> Self {
        Self::Success
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::Error(message.into())
    }
}

#[derive(Debug, Default)]
pub struct ChatCommandControl {
    exit_requested: bool,
}

impl ChatCommandControl {
    pub fn request_exit(&mut self) {
        self.exit_requested = true;
    }

    pub fn exit_requested(&self) -> bool {
        self.exit_requested
    }
}

pub struct ChatCommand {
    pub name: &'static str,
    pub usage: &'static str,
    pub summary: &'static str,
    pub execute: ChatCommandHandler,
}

impl Clone for ChatCommand {
    fn clone(&self) -> Self {
        *self
    }
}

impl Copy for ChatCommand {}

pub struct ChatCommandContext<'a> {
    pub model: &'a mut ChatModel,
    pub renderer: &'a Renderer,
    pub tools: &'a ToolStorage,
    runner: &'a dyn ChatTurnRunner,
    control: &'a mut ChatCommandControl,
}

impl<'a> ChatCommandContext<'a> {
    pub fn new(
        model: &'a mut ChatModel,
        renderer: &'a Renderer,
        tools: &'a ToolStorage,
        runner: &'a dyn ChatTurnRunner,
        control: &'a mut ChatCommandControl,
    ) -> Self {
        Self {
            model,
            renderer,
            tools,
            runner,
            control,
        }
    }

    #[allow(
        dead_code,
        reason = "command context exposes persistence for commands that need history records"
    )]
    pub fn append_agent_event(&self, event: &AgentEvent) -> Result<(), ChatError> {
        self.model.append_agent_event(event)
    }

    pub async fn render_records(&self, records: &[ChatRecord]) -> Result<(), ChatError> {
        self.renderer.render_records(records, self.tools).await
    }

    pub fn render_history(&self, table: &HistoryTableModel) {
        self.renderer.history_table(table);
    }

    pub fn clear_screen(&self) {
        self.renderer.clear_screen();
    }

    pub fn session_created(&self, id: &str) {
        self.renderer.session_created(id);
    }

    pub fn session_resumed(&self, id: &str) {
        self.renderer.resumed(id);
    }

    pub fn notice(&self, message: &str) {
        self.renderer.dim(message);
    }

    pub fn success(&self, message: &str) {
        self.renderer.success(message);
    }

    pub fn request_exit(&mut self) {
        self.control.request_exit();
    }

    pub async fn run_prompt(&mut self, request: ChatRunRequestModel) -> Result<(), ChatError> {
        self.runner
            .run(self.model, self.renderer, self.tools, request)
            .await
    }
}

pub struct ChatCommandAdapter {
    commands: BTreeMap<&'static str, ChatCommand>,
    metadata: Arc<CommandRegistry<()>>,
}

impl ChatCommandAdapter {
    pub fn new<const N: usize>(commands: [ChatCommand; N]) -> Result<Self, CommandError> {
        let mut handlers = BTreeMap::new();
        let mut metadata = CommandRegistry::new();
        for command in commands {
            metadata.register(Command {
                name: command.name,
                usage: command.usage,
                summary: command.summary,
                execute: metadata_execute,
            })?;
            handlers.insert(command.name, command);
        }

        Ok(Self {
            commands: handlers,
            metadata: Arc::new(metadata),
        })
    }

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

    pub fn metadata(&self) -> &Arc<CommandRegistry<()>> {
        &self.metadata
    }
}

fn metadata_execute<'a>(_context: &'a mut (), _args: Vec<String>) -> CommandFuture<'a> {
    Box::pin(async { Ok(CommandControl::Continue) })
}

pub fn registry() -> Result<ChatCommandAdapter, CommandError> {
    ChatCommandAdapter::new([
        session::new::command(),
        session::history::command(),
        session::resume::command(),
        session::clear::command(),
        session::exit::command(),
        config::provider::command(),
        config::model::command(),
        config::reasoning::command(),
        runtime::retry::command(),
        git::status::command(),
        git::commit::command(),
    ])
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use crate::chat::runner::{ChatTurnFuture, ChatTurnRunner};

    pub(crate) struct NoopRunner;

    impl ChatTurnRunner for NoopRunner {
        fn run<'a>(
            &'a self,
            _model: &'a mut ChatModel,
            _renderer: &'a Renderer,
            _tools: &'a ToolStorage,
            _request: ChatRunRequestModel,
        ) -> ChatTurnFuture<'a> {
            Box::pin(async { Ok(()) })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::commands::test_support::NoopRunner;
    use crate::chat::RuntimeSelection;
    use spectacular_agent::AgentEvent;
    use spectacular_config::ReasoningLevel;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[tokio::test]
    async fn adapter_executes_registered_command_success() {
        let adapter = ChatCommandAdapter::new([session::clear::command()]).unwrap();
        let mut model = test_model();
        let renderer = Renderer::default();
        let tools = ToolStorage::default();
        let runner = NoopRunner;
        let mut control = ChatCommandControl::default();
        let context = ChatCommandContext::new(&mut model, &renderer, &tools, &runner, &mut control);

        let result = adapter
            .execute(
                context,
                CommandInvocation {
                    name: "clear".to_owned(),
                    args: Vec::new(),
                },
            )
            .await;

        assert_eq!(result, ChatCommandResult::Success);
    }

    #[test]
    fn registry_exposes_command_metadata() {
        let adapter = registry().unwrap();

        assert!(adapter
            .metadata()
            .search("", 16)
            .iter()
            .any(|entry| entry.metadata.name == "retry"));
    }

    #[test]
    fn context_append_agent_event_persists_chat_record() {
        let mut model = test_model();
        let renderer = Renderer::default();
        let tools = ToolStorage::default();
        let runner = NoopRunner;
        let mut control = ChatCommandControl::default();
        {
            let context =
                ChatCommandContext::new(&mut model, &renderer, &tools, &runner, &mut control);

            context
                .append_agent_event(&AgentEvent::UserPrompt {
                    content: "persist me".to_owned(),
                })
                .unwrap();
        }

        assert!(model.records().unwrap().iter().any(|record| matches!(
            record.event(),
            Some(crate::chat::session::ChatEvent::UserPrompt { content, .. })
                if content == "persist me"
        )));
    }

    #[tokio::test]
    async fn context_render_records_accepts_transient_records() {
        let mut model = test_model();
        let renderer = Renderer::default();
        let tools = ToolStorage::default();
        let runner = NoopRunner;
        let mut control = ChatCommandControl::default();
        let context = ChatCommandContext::new(&mut model, &renderer, &tools, &runner, &mut control);

        context.render_records(&[]).await.unwrap();
    }

    #[test]
    fn context_render_history_accepts_transient_history() {
        let mut model = test_model();
        let renderer = Renderer::default();
        let tools = ToolStorage::default();
        let table = model
            .history(crate::chat::session::HistoryQuery::FirstPage)
            .unwrap();
        let runner = NoopRunner;
        let mut control = ChatCommandControl::default();
        let context = ChatCommandContext::new(&mut model, &renderer, &tools, &runner, &mut control);

        context.render_history(&table);
    }

    fn test_model() -> ChatModel {
        let session = crate::chat::session::SessionManager::new_in(temp_session_dir("adapter"))
            .expect("session manager should be created");
        let mut model = ChatModel::new(
            session,
            RuntimeSelection {
                provider: "openrouter".to_owned(),
                api_key: "sk-or-v1-test".to_owned(),
                model: "test/model".to_owned(),
                reasoning: ReasoningLevel::Medium,
            },
        );
        model.start_new_session().unwrap();
        model
    }

    fn temp_session_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        std::env::temp_dir().join(format!("spectacular-command-adapter-{name}-{suffix}"))
    }
}
