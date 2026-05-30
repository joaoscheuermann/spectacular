use ::config::{ReasoningLevel, TaskModelSlot};
use clap::{Args, Parser, Subcommand};

#[derive(Debug, Eq, PartialEq, Parser)]
#[command(name = "doric")]
#[command(about = "Spec Driven Development workflow assistant")]
pub(super) struct Cli {
    #[command(subcommand)]
    pub(super) command: Option<Command>,
}

#[derive(Debug, Eq, PartialEq, Subcommand)]
pub(super) enum Command {
    /// Inspect or update Doric configuration.
    Config(ConfigArgs),
    /// Start the local lifecycle daemon.
    Daemon(LifecycleDaemonArgs),
    /// Start a feature prompt/requirements lifecycle run.
    Feature(LifecycleDispatchArgs),
    /// Start a debug prompt/requirements lifecycle run.
    Debug(LifecycleDispatchArgs),
    /// List lifecycle workers and requests.
    List(LifecycleAddressArgs),
    /// Inspect lifecycle worker state.
    Worker(LifecycleWorkerArgs),
    /// Answer a lifecycle worker request.
    Answer(LifecycleAnswerArgs),
}

#[derive(Debug, Eq, PartialEq, Args)]
pub(super) struct ConfigArgs {
    #[command(subcommand)]
    pub(super) command: Option<ConfigCommand>,
}

#[derive(Debug, Eq, PartialEq, Args)]
pub(super) struct LifecycleAddressArgs {
    /// Daemon address to route this lifecycle command to.
    #[arg(long)]
    pub(super) addr: Option<String>,
}

#[derive(Debug, Eq, PartialEq, Args)]
pub(super) struct LifecycleDaemonArgs {
    /// Daemon address to route this lifecycle command to.
    #[arg(long)]
    pub(super) addr: Option<String>,
    /// Worker root directory for daemon-managed lifecycle runs.
    #[arg(long)]
    pub(super) worker_root: Option<String>,
}

#[derive(Debug, Eq, PartialEq, Args)]
pub(super) struct LifecycleDispatchArgs {
    /// Lifecycle prompt to hand to the daemon.
    #[arg(long)]
    pub(super) prompt: String,
    /// Repository path for the lifecycle run.
    #[arg(long)]
    pub(super) repo: String,
    /// Daemon address to route this lifecycle command to.
    #[arg(long)]
    pub(super) addr: Option<String>,
}

#[derive(Debug, Eq, PartialEq, Args)]
pub(super) struct LifecycleAnswerArgs {
    pub(super) worker_id: String,
    pub(super) request_id: String,
    /// Answer text for the worker request.
    #[arg(long)]
    pub(super) text: String,
    /// Daemon address to route this lifecycle command to.
    #[arg(long)]
    pub(super) addr: Option<String>,
}

#[derive(Debug, Eq, PartialEq, Args)]
pub(super) struct LifecycleWorkerArgs {
    pub(super) id: String,
    /// Daemon address to route this lifecycle command to.
    #[arg(long)]
    pub(super) addr: Option<String>,
}

#[derive(Debug, Eq, PartialEq, Subcommand)]
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

#[derive(Debug, Eq, PartialEq, Subcommand)]
pub(super) enum ConfigProviderCommand {
    /// Add a provider with fields: provider:<type> apikey:<apikey>.
    Add { fields: Vec<String> },
    /// Remove a provider with fields: name:<name> confirm:true.
    Remove { fields: Vec<String> },
}

#[derive(Debug, Eq, PartialEq, Subcommand)]
pub(super) enum ConfigModelCommand {
    /// Add a saved model with fields: provider:<name> id:<model-id> reasoning:<level> [name:<key>].
    Add { fields: Vec<String> },
    /// Edit a saved model with fields: name:<key> [provider:<name>] [id:<model-id>] [reasoning:<level>].
    Edit { fields: Vec<String> },
    /// Remove a saved model with fields: name:<key> confirm:true.
    Remove { fields: Vec<String> },
}

#[derive(Debug, Eq, PartialEq, Subcommand)]
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
