/// Defines whether a completed field value should be checked against its suggested values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CompletionValueValidation {
    None,
    OneOfValues,
}

/// Describes a named command field for TUI completion metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompletionFieldSpec {
    pub name: &'static str,
    pub summary: &'static str,
    pub required: bool,
    pub validation: CompletionValueValidation,
}

/// Describes a completable subcommand and the fields it accepts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompletionSubcommandSpec {
    pub name: &'static str,
    pub summary: &'static str,
    pub fields: &'static [CompletionFieldSpec],
}

/// Describes a command with subcommand-aware completion metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompletionCommandSpec {
    pub name: &'static str,
    pub subcommands: &'static [CompletionSubcommandSpec],
}
