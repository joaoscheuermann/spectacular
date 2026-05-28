use ::config::{ReasoningLevel, TaskModelSlot};
use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "doric")]
#[command(about = "Spec Driven Development workflow assistant")]
pub(super) struct Cli {
    #[command(subcommand)]
    pub(super) command: Option<Command>,
}
#[derive(Debug, Subcommand)]
pub(super) enum Command {
    /// Inspect or update Doric configuration.
    Config(ConfigArgs),
}

#[derive(Debug, Args)]
pub(super) struct ConfigArgs {
    #[command(subcommand)]
    pub(super) command: Option<ConfigCommand>,
}

#[derive(Debug, Subcommand)]
pub(super) enum ConfigCommand {
    Provider {
        #[command(subcommand)]
        command: ConfigProviderCommand,
    },
    Model {
        #[command(subcommand)]
        command: ConfigModelCommand,
    },
    Task {
        #[command(subcommand)]
        command: ConfigTaskCommand,
    },
}

#[derive(Debug, Subcommand)]
pub(super) enum ConfigProviderCommand {
    /// Add a provider with fields: provider:<type> apikey:<apikey>.
    Add { fields: Vec<String> },
    /// Remove a provider with fields: name:<name> confirm:true.
    Remove { fields: Vec<String> },
}

#[derive(Debug, Subcommand)]
pub(super) enum ConfigModelCommand {
    /// Add a saved model with fields: provider:<name> id:<model-id> reasoning:<level> [name:<key>].
    Add { fields: Vec<String> },
    /// Edit a saved model with fields: name:<key> [provider:<name>] [id:<model-id>] [reasoning:<level>].
    Edit { fields: Vec<String> },
    /// Remove a saved model with fields: name:<key> confirm:true.
    Remove { fields: Vec<String> },
}

#[derive(Debug, Subcommand)]
pub(super) enum ConfigTaskCommand {
    /// Assign a task with fields: task:<general|coding|labeling> model:<model-key>.
    Set { fields: Vec<String> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ConfigOperation {
    Show,
    AddProvider {
        provider_type: String,
        apikey: String,
    },
    RemoveProvider {
        name: String,
        confirm: bool,
    },
    AddModel {
        provider: String,
        model_id: String,
        reasoning: ReasoningLevel,
        name: Option<String>,
    },
    EditModel {
        name: String,
        provider: Option<String>,
        model_id: Option<String>,
        reasoning: Option<ReasoningLevel>,
    },
    RemoveModel {
        name: String,
        confirm: bool,
    },
    SetTask {
        task: TaskModelSlot,
        model: String,
    },
}
