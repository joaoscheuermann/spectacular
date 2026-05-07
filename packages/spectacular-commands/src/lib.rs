use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display};
use std::future::Future;
use std::pin::Pin;

pub type CommandFuture<'a> =
    Pin<Box<dyn Future<Output = Result<CommandControl, CommandError>> + Send + 'a>>;

pub type CommandHandler<C> = for<'a> fn(&'a mut C, Vec<String>) -> CommandFuture<'a>;

pub struct Command<C> {
    pub name: &'static str,
    pub usage: &'static str,
    pub summary: &'static str,
    pub execute: CommandHandler<C>,
}

impl<C> Clone for Command<C> {
    /// Provides the clone behavior for slash-command parsing and completion.
    fn clone(&self) -> Self {
        *self
    }
}

impl<C> Copy for Command<C> {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommandMetadata {
    pub name: &'static str,
    pub usage: &'static str,
    pub summary: &'static str,
}

impl<C> Command<C> {
    /// Provides the metadata behavior for slash-command parsing and completion.
    pub fn metadata(self) -> CommandMetadata {
        CommandMetadata {
            name: self.name,
            usage: self.usage,
            summary: self.summary,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommandSearchMatch {
    pub metadata: CommandMetadata,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompletionCommandSpec {
    pub name: &'static str,
    pub subcommands: &'static [CompletionSubcommandSpec],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompletionSubcommandSpec {
    pub name: &'static str,
    pub summary: &'static str,
    pub fields: &'static [CompletionFieldSpec],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompletionFieldSpec {
    pub name: &'static str,
    pub summary: &'static str,
    pub required: bool,
    pub value_source: CompletionValueSource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionValueSource {
    Static(&'static [&'static str]),
    Dynamic(&'static str),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamedArgs {
    values: BTreeMap<String, String>,
}

impl NamedArgs {
    /// Provides the parse behavior for slash-command parsing and completion.
    pub fn parse(tokens: &[String]) -> Result<Self, CommandError> {
        let mut values = BTreeMap::new();
        for token in tokens {
            let Some((field, value)) = token.split_once(':') else {
                return Err(CommandError::InvalidNamedArgument {
                    argument: token.clone(),
                });
            };
            if field.trim().is_empty() {
                return Err(CommandError::InvalidNamedArgument {
                    argument: token.clone(),
                });
            }
            if values.insert(field.to_owned(), value.to_owned()).is_some() {
                return Err(CommandError::DuplicateNamedArgument {
                    name: field.to_owned(),
                });
            }
        }

        Ok(Self { values })
    }

    /// Provides the require behavior for slash-command parsing and completion.
    pub fn require(&self, name: &'static str) -> Result<&str, CommandError> {
        self.get(name)
            .filter(|value| !value.trim().is_empty())
            .ok_or(CommandError::MissingNamedArgument { name })
    }

    /// Provides the optional behavior for slash-command parsing and completion.
    pub fn optional(&self, name: &str) -> Option<&str> {
        self.get(name).filter(|value| !value.trim().is_empty())
    }

    /// Provides the get behavior for slash-command parsing and completion.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    /// Provides the reject unknown behavior for slash-command parsing and completion.
    pub fn reject_unknown(&self, allowed: &[&str]) -> Result<(), CommandError> {
        let Some(name) = self
            .values
            .keys()
            .find(|name| !allowed.contains(&name.as_str()))
        else {
            return Ok(());
        };

        Err(CommandError::UnknownNamedArgument { name: name.clone() })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandControl {
    Continue,
    Exit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandInvocation {
    pub name: String,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParseOutcome {
    NotCommand,
    Command(CommandInvocation),
}

#[derive(Default)]
pub struct CommandRegistry<C> {
    commands: BTreeMap<&'static str, Command<C>>,
}

impl<C> CommandRegistry<C> {
    /// Provides the new behavior for slash-command parsing and completion.
    pub fn new() -> Self {
        Self {
            commands: BTreeMap::new(),
        }
    }

    /// Provides the register behavior for slash-command parsing and completion.
    pub fn register(&mut self, command: Command<C>) -> Result<(), CommandError> {
        if command.name.trim().is_empty() {
            return Err(CommandError::InvalidRegistration {
                message: "command name cannot be empty".to_owned(),
            });
        }

        if command.name.chars().any(|character| {
            character.is_ascii_uppercase() || character.is_whitespace() || character == '/'
        }) {
            return Err(CommandError::InvalidRegistration {
                message: format!("invalid command name `{}`", command.name),
            });
        }

        if self.commands.contains_key(command.name) {
            return Err(CommandError::DuplicateCommand {
                name: command.name.to_owned(),
            });
        }

        self.commands.insert(command.name, command);
        Ok(())
    }

    /// Provides the with behavior for slash-command parsing and completion.
    pub fn with(mut self, command: Command<C>) -> Result<Self, CommandError> {
        self.register(command)?;
        Ok(self)
    }

    pub async fn execute(
        &self,
        context: &mut C,
        invocation: CommandInvocation,
    ) -> Result<CommandControl, CommandError> {
        let Some(command) = self.commands.get(invocation.name.as_str()) else {
            return Err(CommandError::UnknownCommand {
                name: invocation.name,
            });
        };

        (command.execute)(context, invocation.args).await
    }

    /// Provides the commands behavior for slash-command parsing and completion.
    pub fn commands(&self) -> impl Iterator<Item = &Command<C>> {
        self.commands.values()
    }

    /// Provides the command metadata behavior for slash-command parsing and completion.
    pub fn command_metadata(&self) -> impl Iterator<Item = CommandMetadata> + '_ {
        self.commands.values().map(|command| command.metadata())
    }

    /// Provides the search behavior for slash-command parsing and completion.
    pub fn search(&self, query: &str, limit: usize) -> Vec<CommandSearchMatch> {
        let mut matches = self
            .commands
            .values()
            .filter_map(|command| {
                let rank = fuzzy_rank(command.name, query)?;
                Some((rank, command.metadata()))
            })
            .collect::<Vec<_>>();

        matches.sort_by(|(left_rank, left), (right_rank, right)| {
            left_rank
                .cmp(right_rank)
                .then_with(|| left.name.cmp(right.name))
        });

        matches
            .into_iter()
            .take(limit)
            .map(|(_, metadata)| CommandSearchMatch { metadata })
            .collect()
    }
}

/// Provides the parse line behavior for slash-command parsing and completion.
pub fn parse_line(line: &str) -> Result<ParseOutcome, CommandError> {
    if !line.starts_with('/') {
        return Ok(ParseOutcome::NotCommand);
    }

    let body = &line[1..];
    if body.trim().is_empty() {
        return Err(CommandError::EmptyCommand);
    }

    let tokens = shell_words(body)?;
    let Some((name, args)) = tokens.split_first() else {
        return Err(CommandError::EmptyCommand);
    };

    if name.chars().any(|character| character.is_ascii_uppercase()) {
        return Err(CommandError::UnknownCommand { name: name.clone() });
    }

    Ok(ParseOutcome::Command(CommandInvocation {
        name: name.clone(),
        args: args.to_vec(),
    }))
}

/// Provides the shell words behavior for slash-command parsing and completion.
fn shell_words(input: &str) -> Result<Vec<String>, CommandError> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    let mut in_quotes = false;
    let mut token_started = false;

    while let Some(character) = chars.next() {
        match character {
            '"' => {
                in_quotes = !in_quotes;
                token_started = true;
            }
            '\\' if in_quotes => {
                let Some(next) = chars.next() else {
                    current.push('\\');
                    continue;
                };
                current.push(next);
                token_started = true;
            }
            character if character.is_whitespace() && !in_quotes => {
                if token_started {
                    tokens.push(std::mem::take(&mut current));
                    token_started = false;
                }
            }
            _ => {
                current.push(character);
                token_started = true;
            }
        }
    }

    if in_quotes {
        return Err(CommandError::UnterminatedQuote);
    }

    if token_started {
        tokens.push(current);
    }

    Ok(tokens)
}

/// Provides the fuzzy rank behavior for slash-command parsing and completion.
pub fn fuzzy_rank(name: &str, query: &str) -> Option<(u8, usize)> {
    if query.is_empty() {
        return Some((3, 0));
    }

    if query.chars().any(char::is_uppercase) {
        return None;
    }

    if name == query {
        return Some((0, 0));
    }

    if name.starts_with(query) {
        return Some((1, name.len().saturating_sub(query.len())));
    }

    let distance = levenshtein(name, query);
    (distance <= 2).then_some((2, distance))
}

/// Provides the fuzzy filter behavior for slash-command parsing and completion.
pub fn fuzzy_filter<'a>(
    candidates: impl IntoIterator<Item = &'a str>,
    query: &str,
    limit: usize,
) -> Vec<&'a str> {
    let mut matches = candidates
        .into_iter()
        .filter_map(|candidate| fuzzy_rank(candidate, query).map(|rank| (rank, candidate)))
        .collect::<Vec<_>>();

    matches.sort_by(|(left_rank, left), (right_rank, right)| {
        left_rank.cmp(right_rank).then_with(|| left.cmp(right))
    });

    matches
        .into_iter()
        .take(limit)
        .map(|(_, candidate)| candidate)
        .collect()
}

/// Provides the levenshtein behavior for slash-command parsing and completion.
fn levenshtein(left: &str, right: &str) -> usize {
    if left == right {
        return 0;
    }

    if left.is_empty() {
        return right.chars().count();
    }
    if right.is_empty() {
        return left.chars().count();
    }

    let right_chars = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right_chars.len()).collect::<Vec<_>>();
    let mut current = vec![0; right_chars.len() + 1];

    for (left_index, left_char) in left.chars().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_char) in right_chars.iter().enumerate() {
            let substitution = previous[right_index] + usize::from(left_char != *right_char);
            let insertion = current[right_index] + 1;
            let deletion = previous[right_index + 1] + 1;
            current[right_index + 1] = substitution.min(insertion).min(deletion);
        }
        std::mem::swap(&mut previous, &mut current);
    }

    previous[right_chars.len()]
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandError {
    EmptyCommand,
    UnknownCommand { name: String },
    UnterminatedQuote,
    DuplicateCommand { name: String },
    InvalidRegistration { message: String },
    InvalidNamedArgument { argument: String },
    DuplicateNamedArgument { name: String },
    MissingNamedArgument { name: &'static str },
    UnknownNamedArgument { name: String },
    Usage { usage: String },
    Message(String),
}

impl CommandError {
    /// Provides the usage behavior for slash-command parsing and completion.
    pub fn usage(usage: impl Into<String>) -> Self {
        Self::Usage {
            usage: usage.into(),
        }
    }

    /// Provides the message behavior for slash-command parsing and completion.
    pub fn message(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}

impl Display for CommandError {
    /// Provides the fmt behavior for slash-command parsing and completion.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::EmptyCommand => formatter.write_str("empty command"),
            CommandError::UnknownCommand { name } => write!(formatter, "unknown command /{name}"),
            CommandError::UnterminatedQuote => formatter.write_str("unterminated quote"),
            CommandError::DuplicateCommand { name } => {
                write!(formatter, "duplicate command `{name}`")
            }
            CommandError::InvalidRegistration { message } => formatter.write_str(message),
            CommandError::InvalidNamedArgument { argument } => write!(
                formatter,
                "invalid argument `{argument}`; expected name:<value>"
            ),
            CommandError::DuplicateNamedArgument { name } => {
                write!(formatter, "duplicate argument `{name}`")
            }
            CommandError::MissingNamedArgument { name } => {
                write!(formatter, "missing required argument `{name}:<value>`")
            }
            CommandError::UnknownNamedArgument { name } => {
                write!(formatter, "unknown argument `{name}`")
            }
            CommandError::Usage { usage } => write!(formatter, "usage: {usage}"),
            CommandError::Message(message) => formatter.write_str(message),
        }
    }
}

impl Error for CommandError {}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/commands.rs"
    ));
}
