use super::compact::CompactTerminalOutput;
use super::diagnostics::Diagnostic;
use super::TerminalExecution;

/// Parsed command metadata used to choose deterministic terminal reducers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CommandSummary {
    pub original: String,
    pub executable: String,
    pub args: Vec<String>,
}

impl CommandSummary {
    /// Parses the command with simple shell tokenization and common environment-prefix handling.
    pub(crate) fn parse(command: &str) -> Self {
        let tokens = tokenize_command(command);
        let command_start = tokens
            .iter()
            .position(|token| !looks_like_command_prefix(token))
            .unwrap_or(0);
        let executable = tokens.get(command_start).cloned().unwrap_or_default();
        let args = tokens
            .iter()
            .skip(command_start + 1)
            .cloned()
            .collect::<Vec<_>>();

        Self {
            original: command.to_owned(),
            executable: normalize_executable(&executable),
            args,
        }
    }
}

/// Reducer contract for command-specific terminal output enrichment.
pub(crate) trait TerminalReducer {
    /// Reports whether this reducer applies to the parsed command summary.
    fn matches(&self, command: &CommandSummary) -> bool;

    /// Enriches the generic compact output while keeping the generic fallback intact.
    fn reduce(
        &self,
        execution: &TerminalExecution,
        generic: CompactTerminalOutput,
    ) -> CompactTerminalOutput;
}

/// Applies the built-in reducer chain to a generic compact terminal output.
pub(crate) fn reduce_terminal_output(
    execution: &TerminalExecution,
    generic: CompactTerminalOutput,
) -> CompactTerminalOutput {
    let cargo = CargoReducer;
    reduce_with_reducers(execution, generic, &[&cargo])
}

/// Applies the first matching reducer from an injected reducer list.
pub(crate) fn reduce_with_reducers(
    execution: &TerminalExecution,
    generic: CompactTerminalOutput,
    reducers: &[&dyn TerminalReducer],
) -> CompactTerminalOutput {
    let summary = CommandSummary::parse(&execution.command);
    let Some(reducer) = reducers.iter().find(|reducer| reducer.matches(&summary)) else {
        return generic;
    };

    reducer.reduce(execution, generic)
}

struct CargoReducer;

impl TerminalReducer for CargoReducer {
    /// Reports whether a command is a supported Cargo verification command.
    fn matches(&self, command: &CommandSummary) -> bool {
        if command.executable != "cargo" {
            return false;
        }

        command
            .args
            .first()
            .is_some_and(|arg| matches!(arg.as_str(), "test" | "check" | "clippy"))
    }

    /// Adds Cargo-specific failed-test diagnostics without removing generic diagnostics.
    fn reduce(
        &self,
        execution: &TerminalExecution,
        mut generic: CompactTerminalOutput,
    ) -> CompactTerminalOutput {
        append_failed_cargo_tests(&mut generic.diagnostics, &execution.stdout);
        generic
    }
}

/// Adds failed Rust test names from Cargo test output to the diagnostics list.
fn append_failed_cargo_tests(diagnostics: &mut Vec<Diagnostic>, stdout: &str) {
    for (line_index, line) in stdout.lines().enumerate() {
        if !line.starts_with("test ") || !line.contains("FAILED") {
            continue;
        }

        push_unique(
            diagnostics,
            Diagnostic {
                kind: "cargo_test_failure".to_owned(),
                stream: "stdout".to_owned(),
                line: line_index + 1,
                text: line.to_owned(),
                context: Vec::new(),
                repeat_count: None,
            },
        );
    }
}

/// Pushes a diagnostic only when an exact kind, stream, and text match is absent.
fn push_unique(diagnostics: &mut Vec<Diagnostic>, diagnostic: Diagnostic) {
    if diagnostics.iter().any(|existing| {
        existing.kind == diagnostic.kind
            && existing.stream == diagnostic.stream
            && existing.text == diagnostic.text
    }) {
        return;
    }

    diagnostics.push(diagnostic);
}

/// Tokenizes a command line enough for reducer selection without becoming a shell parser.
fn tokenize_command(command: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote = None;

    for character in command.chars() {
        match (quote, character) {
            (Some(active), value) if value == active => quote = None,
            (None, '"' | '\'') => quote = Some(character),
            (None, ';') => {
                push_current_token(&mut tokens, &mut current);
                tokens.push(";".to_owned());
            }
            (None, value) if value.is_whitespace() => {
                push_current_token(&mut tokens, &mut current);
            }
            _ => current.push(character),
        }
    }

    push_current_token(&mut tokens, &mut current);

    tokens
}

/// Pushes the current token into the token list when it has content.
fn push_current_token(tokens: &mut Vec<String>, current: &mut String) {
    if current.is_empty() {
        return;
    }

    tokens.push(std::mem::take(current));
}

/// Reports whether a token is command metadata before the executable.
fn looks_like_command_prefix(token: &str) -> bool {
    is_command_separator(token)
        || looks_like_env_prefix(token)
        || looks_like_powershell_env_prefix(token)
}

/// Reports whether a token separates command statements before the executable.
fn is_command_separator(token: &str) -> bool {
    token == ";"
}

/// Reports whether a token looks like a POSIX-style environment assignment prefix.
fn looks_like_env_prefix(token: &str) -> bool {
    let Some((name, value)) = token.split_once('=') else {
        return false;
    };

    !name.is_empty()
        && !value.is_empty()
        && name
            .chars()
            .all(|character| character.is_ascii_uppercase() || character == '_')
}

/// Reports whether a token looks like a PowerShell `$env:NAME=value` assignment.
fn looks_like_powershell_env_prefix(token: &str) -> bool {
    let Some((name, value)) = token.split_once('=') else {
        return false;
    };
    let prefix_len = "$env:".len();
    if name.len() <= prefix_len || !name[..prefix_len].eq_ignore_ascii_case("$env:") {
        return false;
    }

    !value.is_empty()
}

/// Normalizes executable names across direct calls and Windows `.exe` suffixes.
fn normalize_executable(executable: &str) -> String {
    let file_name = executable.rsplit(['/', '\\']).next().unwrap_or(executable);

    file_name
        .strip_suffix(".exe")
        .unwrap_or(file_name)
        .to_ascii_lowercase()
}
