pub struct PromptEditor<'a, C> {
    renderer: &'a Renderer,
    registry: &'a Arc<CommandRegistry<C>>,
    completions: &'a PromptCompletionCatalog<'a>,
    footer: Option<ChatPromptFooterModel>,
    state: PromptState,
    terminal: PromptTerminal,
    rendered_lines: u16,
    rendered_cursor_row: u16,
    paste_burst: PasteBurst,
}

pub(crate) struct PromptCompletionCatalog<'a> {
    specs: &'a [CompletionCommandSpec],
    environment: Option<CompletionEnvironment>,
}

impl<'a> PromptCompletionCatalog<'a> {
    /// Creates a completion catalog backed by command specs and the active chat model.
    pub(crate) fn new(specs: &'a [CompletionCommandSpec], model: &ChatModel) -> Self {
        Self {
            specs,
            environment: Some(model.completion_environment()),
        }
    }

    /// Returns command metadata for a registered completion command.
    fn spec(&self, command: &str) -> Option<CompletionCommandSpec> {
        self.specs.iter().copied().find(|spec| spec.name == command)
    }

    /// Returns subcommand metadata for a registered command path.
    fn subcommand(&self, command: &str, subcommand: &str) -> Option<CompletionSubcommandSpec> {
        find_subcommand(self.spec(command)?.subcommands, subcommand)
    }

    /// Returns field metadata for a registered command/subcommand/field path.
    fn field(&self, command: &str, subcommand: &str, field: &str) -> Option<CompletionFieldSpec> {
        find_field(self.subcommand(command, subcommand)?.fields, field)
    }

    /// Resolves values through the field-owned callback using the active prompt state.
    fn resolve_values(
        &self,
        field: CompletionFieldSpec,
        subcommand: &str,
        pairs: &[(String, String)],
    ) -> Result<Vec<String>, ChatError> {
        let environment = self.environment.ok_or_else(|| {
            ChatError::Session("completion catalog has no environment".to_owned())
        })?;
        let context = crate::chat::commands::ChatCompletionContext::new(
            environment,
            subcommand,
            pairs,
        );

        (field.values)(&context)
    }

    /// Validates a closed-choice field value against the same resolver used for suggestions.
    fn validate_choice(
        &self,
        field: CompletionFieldSpec,
        value: &str,
        subcommand: &str,
        pairs: &[(String, String)],
    ) -> Result<ChoiceValidation, ChatError> {
        let allowed = self.resolve_values(field, subcommand, pairs)?;
        if allowed.is_empty() || allowed.iter().any(|allowed| allowed == value) {
            return Ok(ChoiceValidation::Valid);
        }

        Ok(ChoiceValidation::Invalid(allowed))
    }
}

impl Default for PromptCompletionCatalog<'_> {
    /// Creates an empty catalog for tests that only exercise command-name completion.
    fn default() -> Self {
        Self {
            specs: &[],
            environment: None,
        }
    }
}

/// Result of validating a field value against closed-choice completion values.
#[derive(Clone, Debug, Eq, PartialEq)]
enum ChoiceValidation {
    Valid,
    Invalid(Vec<String>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PromptSuggestion {
    replacement: String,
    label: String,
    summary: String,
    append_space: bool,
    kind: PromptSuggestionKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PromptSuggestionKind {
    Command,
    Subcommand,
    Field,
    Value,
    Info,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PromptGuidanceLine {
    Missing(Vec<String>),
    Detail(String),
    Info(String),
}

#[derive(Default)]
struct PromptState {
    buffer: String,
    cursor: usize,
    selection_anchor: Option<usize>,
    selected: usize,
    preferred_column: Option<usize>,
    kill_buffer: String,
    dismissed_completion: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct VisualRow {
    start: usize,
    end: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CursorPosition {
    row: usize,
    column: usize,
}
