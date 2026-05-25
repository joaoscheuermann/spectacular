use super::completion::CompletionSubcommandSpec;
use super::context::ChatCommandContext;
use crate::chat::model::ChatRunRequestModel;
use std::future::Future;
use std::pin::Pin;

pub type ChatCommandFuture<'a> = Pin<Box<dyn Future<Output = ChatCommandResult> + 'a>>;

pub type ChatCommandHandler =
    for<'a> fn(ChatCommandContext<'a>, Vec<String>) -> ChatCommandFuture<'a>;

#[derive(Debug, Eq, PartialEq)]
pub enum ChatCommandResult {
    Success,
    Error(String),
}

impl ChatCommandResult {
    /// Creates a successful command result with no extra control flow.
    pub fn success() -> Self {
        Self::Success
    }

    /// Creates a command error result with user-facing text.
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error(message.into())
    }
}

#[derive(Debug, Default)]
pub struct ChatCommandControl {
    exit_requested: bool,
    follow_up_prompt: Option<ChatRunRequestModel>,
}

impl ChatCommandControl {
    /// Marks that the command loop should exit after the current command finishes.
    pub fn request_exit(&mut self) {
        self.exit_requested = true;
    }

    /// Returns whether a command requested the chat loop to exit.
    pub fn exit_requested(&self) -> bool {
        self.exit_requested
    }

    /// Requests that the controller run a prompt after the command completes.
    pub fn request_prompt_run(&mut self, request: ChatRunRequestModel) {
        self.follow_up_prompt = Some(request);
    }

    /// Takes the pending follow-up prompt request, if a command queued one.
    pub fn take_follow_up_prompt(&mut self) -> Option<ChatRunRequestModel> {
        self.follow_up_prompt.take()
    }
}

pub struct ChatCommand {
    pub name: &'static str,
    pub usage: &'static str,
    pub summary: &'static str,
    pub completion: &'static [CompletionSubcommandSpec],
    pub execute: ChatCommandHandler,
}

impl Clone for ChatCommand {
    /// Copies static command metadata and handler pointers.
    fn clone(&self) -> Self {
        *self
    }
}

impl Copy for ChatCommand {}
