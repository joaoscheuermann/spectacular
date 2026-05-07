pub struct PromptEditor<'a, C> {
    renderer: &'a Renderer,
    registry: &'a Arc<CommandRegistry<C>>,
    completions: &'a PromptCompletionCatalog,
    state: PromptState,
    terminal: PromptTerminal,
    rendered_lines: u16,
    rendered_cursor_row: u16,
    paste_burst: PasteBurst,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct PromptCompletionCatalog {
    specs: BTreeMap<&'static str, CompletionCommandSpec>,
    sources: BTreeMap<String, Vec<String>>,
}

impl PromptCompletionCatalog {
    pub(crate) fn new(
        specs: &[CompletionCommandSpec],
        sources: BTreeMap<String, Vec<String>>,
    ) -> Self {
        Self {
            specs: specs.iter().map(|spec| (spec.name, *spec)).collect(),
            sources,
        }
    }

    fn spec(&self, command: &str) -> Option<CompletionCommandSpec> {
        self.specs.get(command).copied()
    }

    fn source(&self, name: &str) -> Vec<&str> {
        self.sources
            .get(name)
            .map(|values| values.iter().map(String::as_str).collect())
            .unwrap_or_default()
    }
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
