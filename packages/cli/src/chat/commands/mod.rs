mod adapter;
mod bridge;
pub(crate) mod completion;
mod context;
mod contract;
mod registry;

pub mod config;
pub mod git;
pub mod runtime;
pub mod session;

pub use adapter::ChatCommandAdapter;
pub use context::ChatCommandContext;
#[allow(
    unused_imports,
    reason = "preserves the command module API even when no in-crate caller names the alias directly"
)]
pub use contract::{
    ChatCommand, ChatCommandControl, ChatCommandFuture, ChatCommandHandler, ChatCommandResult,
};
pub use registry::registry;

pub(crate) use completion::{
    CompletionCommandSpec, CompletionFieldSpec, CompletionSubcommandSpec, CompletionValueValidation,
};

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/commands/adapter.rs"
    ));
}
