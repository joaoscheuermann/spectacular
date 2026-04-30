pub mod config;
pub mod runtime;
pub mod session;

use crate::chat::ChatContext;
use spectacular_commands::{CommandError, CommandRegistry};

pub fn registry() -> Result<CommandRegistry<ChatContext>, CommandError> {
    let mut registry = CommandRegistry::new();
    registry.register(session::new::command())?;
    registry.register(session::history::command())?;
    registry.register(session::resume::command())?;
    registry.register(session::clear::command())?;
    registry.register(session::exit::command())?;
    registry.register(config::provider::command())?;
    registry.register(config::model::command())?;
    registry.register(config::reasoning::command())?;
    registry.register(runtime::retry::command())?;
    Ok(registry)
}
