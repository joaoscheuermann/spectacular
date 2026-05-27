use super::{config, git, runtime, session, ChatCommandAdapter};
use spectacular_commands::CommandError;

/// Builds the default registry of chat commands available in the REPL.
pub fn registry() -> Result<ChatCommandAdapter, CommandError> {
    ChatCommandAdapter::new([
        session::new::command(),
        session::history::command(),
        session::resume::command(),
        session::clear::command(),
        session::exit::command(),
        config::provider::command(),
        config::model::command(),
        config::task::command(),
        runtime::retry::command(),
        git::command(),
    ])
}
