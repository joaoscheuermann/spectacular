#[derive(Debug, Parser)]
#[command(name = "spectacular")]
#[command(about = "Spec Driven Development workflow assistant")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Debug, Subcommand)]
enum Command {
    /// Start an Aider-style terminal chat session.
    Chat(ChatArgs),
    /// Inspect or update Spectacular configuration.
    Config(ConfigArgs),
    /// Run the first SDD planning step.
    Plan {
        /// Prompt to plan from.
        prompt: String,
    },
}

#[derive(Debug, Args)]
struct ChatArgs {
    /// Run chat in the experimental IOCraft terminal UI.
    #[arg(long)]
    tui: bool,
}

#[derive(Debug, Args)]
struct ConfigArgs {
    #[command(subcommand)]
    command: Option<ConfigCommand>,
}

#[derive(Debug, Subcommand)]
enum ConfigCommand {
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
enum ConfigProviderCommand {
    /// Add a provider with fields: provider:<type> apikey:<apikey>.
    Add { fields: Vec<String> },
    /// Remove a provider with fields: name:<name> confirm:true.
    Remove { fields: Vec<String> },
}

#[derive(Debug, Subcommand)]
enum ConfigModelCommand {
    /// Add a saved model with fields: provider:<name> id:<model-id> reasoning:<level> [name:<key>].
    Add { fields: Vec<String> },
    /// Edit a saved model with fields: name:<key> [provider:<name>] [id:<model-id>] [reasoning:<level>].
    Edit { fields: Vec<String> },
    /// Remove a saved model with fields: name:<key> confirm:true.
    Remove { fields: Vec<String> },
}

#[derive(Debug, Subcommand)]
enum ConfigTaskCommand {
    /// Assign a task with fields: task:<general|coding|labeling> model:<model-key>.
    Set { fields: Vec<String> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ConfigOperation {
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
