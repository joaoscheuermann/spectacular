use super::bridge::TuiCommandBridge;
use super::contract::ChatCommandControl;
use crate::chat::command_event::{CommandEvent, CommandStatus};
use crate::chat::model::{
    ChatModel, ChatPromptFooterModel, ChatRunRequestModel, HistoryTableModel,
};
use crate::chat::selection::{SelectionPromptAnswer, SelectionPromptRequest};
use crate::chat::session::ChatRecord;
use crate::chat::ChatError;
use spectacular_agent::{AgentEvent, ToolStorage};
use spectacular_tui::ChatTuiAction;
use std::future::Future;
use std::path::Path;

pub struct ChatCommandContext<'a> {
    pub model: &'a mut ChatModel,
    pub tools: &'a ToolStorage,
    control: &'a mut ChatCommandControl,
    tui: Option<TuiCommandBridge<'a>>,
}

impl<'a> ChatCommandContext<'a> {
    /// Creates a command execution context from the active chat services.
    #[allow(
        dead_code,
        reason = "unit tests use contexts without the live TUI bridge"
    )]
    #[cfg(test)]
    pub fn new(
        model: &'a mut ChatModel,
        tools: &'a ToolStorage,
        control: &'a mut ChatCommandControl,
    ) -> Self {
        Self {
            model,
            tools,
            control,
            tui: None,
        }
    }

    /// Creates a command execution context that projects command output into TUI actions.
    pub(crate) fn new_tui(
        model: &'a mut ChatModel,
        tools: &'a ToolStorage,
        control: &'a mut ChatCommandControl,
        _prompt_footer: Option<ChatPromptFooterModel>,
        dispatch: &'a mut (dyn FnMut(ChatTuiAction) + Send),
        selection_receiver: &'a mut tokio::sync::mpsc::UnboundedReceiver<spectacular_tui::Intent>,
    ) -> Self {
        Self {
            model,
            tools,
            control,
            tui: Some(TuiCommandBridge::new(dispatch, selection_receiver)),
        }
    }

    #[allow(
        dead_code,
        reason = "command context exposes persistence for commands that need history records"
    )]
    pub fn append_agent_event(&self, event: &AgentEvent) -> Result<(), ChatError> {
        self.model.append_agent_event(event)
    }

    /// Appends an app-owned command lifecycle event to the active session transcript.
    pub fn append_command_event(&self, event: &CommandEvent) -> Result<(), ChatError> {
        self.model.append_command_event(event)?;
        if let Some(tui) = &self.tui {
            tui.dispatch_command_event(event);
        }
        Ok(())
    }

    /// Replays chat records through the active command display bridge when available.
    pub async fn render_records(&self, records: &[ChatRecord]) -> Result<(), ChatError> {
        if let Some(tui) = &self.tui {
            tui.render_records(records, self.tools);
        }

        Ok(())
    }

    /// Renders a chat history table through the active command display bridge.
    pub fn render_history(&self, table: &HistoryTableModel) {
        if let Some(tui) = &self.tui {
            tui.render_history(table);
        }
    }

    /// Clears the visible transcript in the active command display bridge.
    pub fn clear_screen(&self) {
        if let Some(tui) = &self.tui {
            tui.dispatch(ChatTuiAction::TranscriptCleared);
        }
    }

    /// Renders a session-created notice for a new chat session when a display bridge exists.
    pub fn session_created(&self, id: &str, directory: &Path) {
        if let Some(tui) = &self.tui {
            tui.dispatch(crate::chat::tui::state::session_created_action(
                id, self.model, directory,
            ));
        }
    }

    /// Renders a session-resumed notice for an existing chat session.
    pub fn session_resumed(&self, id: &str) {
        if let Some(tui) = &self.tui {
            tui.dispatch(ChatTuiAction::SessionChanged {
                id: spectacular_tui::SessionId::new(id),
            });
            tui.dispatch(ChatTuiAction::NoticeReported {
                message: format!("resumed session {id}"),
            });
        }
    }

    /// Renders a low-emphasis informational command message.
    pub fn notice(&self, message: &str) {
        if let Some(tui) = &self.tui {
            tui.dispatch(ChatTuiAction::NoticeReported {
                message: message.to_owned(),
            });
        }
    }

    /// Renders a successful command message.
    pub fn success(&self, message: &str) {
        if let Some(tui) = &self.tui {
            tui.dispatch(ChatTuiAction::SuccessReported {
                message: message.to_owned(),
            });
        }
    }

    /// Renders a blank line when the active command output supports line-oriented spacing.
    pub fn blank_line(&self) {}

    /// Renders a command lifecycle start record.
    pub fn command_start(&self, _title: &str, _command: &str) {}

    /// Renders a command lifecycle progress record.
    pub fn command_delta(&self, _content: &str) {}

    /// Renders a command lifecycle completion record.
    pub fn command_finished(&self, _status: CommandStatus, _summary: &str) {}

    /// Renders an interactive option selection prompt and returns the user's answer.
    pub async fn ask(
        &self,
        request: SelectionPromptRequest,
    ) -> Result<SelectionPromptAnswer, ChatError> {
        if let Some(tui) = &self.tui {
            return tui.ask(request).await;
        }

        Err(ChatError::Session(
            "selection prompt requires the TUI command bridge".to_owned(),
        ))
    }

    /// Runs async command work with a transient "working" indicator until the
    /// operation completes.
    pub async fn work<F, T>(&self, f: F) -> T
    where
        F: Future<Output = T>,
    {
        f.await
    }

    /// Requests that the chat command loop exit after this command.
    pub fn request_exit(&mut self) {
        self.control.request_exit();
    }

    /// Requests that the controller run a prompt after command execution.
    pub fn request_prompt_run(&mut self, request: ChatRunRequestModel) {
        self.control.request_prompt_run(request);
    }
}
