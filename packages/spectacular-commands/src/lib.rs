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
    pub fn new() -> Self {
        Self {
            commands: BTreeMap::new(),
        }
    }

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

    pub fn commands(&self) -> impl Iterator<Item = &Command<C>> {
        self.commands.values()
    }

    pub fn command_metadata(&self) -> impl Iterator<Item = CommandMetadata> + '_ {
        self.commands.values().map(|command| command.metadata())
    }

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

fn fuzzy_rank(name: &str, query: &str) -> Option<(u8, usize)> {
    if query.is_empty() {
        return Some((3, 0));
    }

    if name == query {
        return Some((0, 0));
    }

    if name.starts_with(query) {
        return Some((1, name.len().saturating_sub(query.len())));
    }

    subsequence_gap_score(name, query).map(|score| (2, score))
}

fn subsequence_gap_score(name: &str, query: &str) -> Option<usize> {
    let mut score = 0usize;
    let mut last_match = None;
    let mut name_indices = name.char_indices();

    for query_char in query.chars() {
        let Some((index, _)) = name_indices.find(|(_, name_char)| *name_char == query_char) else {
            return None;
        };

        if let Some(last_match) = last_match {
            score += index.saturating_sub(last_match + 1);
        } else {
            score += index;
        }
        last_match = Some(index);
    }

    Some(score + name.len().saturating_sub(query.len()))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandError {
    EmptyCommand,
    UnknownCommand { name: String },
    UnterminatedQuote,
    DuplicateCommand { name: String },
    InvalidRegistration { message: String },
    Usage { usage: String },
    Message(String),
}

impl CommandError {
    pub fn usage(usage: impl Into<String>) -> Self {
        Self::Usage {
            usage: usage.into(),
        }
    }

    pub fn message(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}

impl Display for CommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::EmptyCommand => formatter.write_str("empty command"),
            CommandError::UnknownCommand { name } => write!(formatter, "unknown command /{name}"),
            CommandError::UnterminatedQuote => formatter.write_str("unterminated quote"),
            CommandError::DuplicateCommand { name } => {
                write!(formatter, "duplicate command `{name}`")
            }
            CommandError::InvalidRegistration { message } => formatter.write_str(message),
            CommandError::Usage { usage } => write!(formatter, "usage: {usage}"),
            CommandError::Message(message) => formatter.write_str(message),
        }
    }
}

impl Error for CommandError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_text_is_not_command() {
        assert_eq!(parse_line("hello").unwrap(), ParseOutcome::NotCommand);
    }

    #[test]
    fn empty_slash_is_rejected() {
        assert!(matches!(parse_line("/"), Err(CommandError::EmptyCommand)));
        assert!(matches!(
            parse_line("/   "),
            Err(CommandError::EmptyCommand)
        ));
    }

    #[test]
    fn command_with_quoted_args_parses() {
        let parsed = parse_line(r#"/model "openai/gpt 4" medium"#).unwrap();

        assert_eq!(
            parsed,
            ParseOutcome::Command(CommandInvocation {
                name: "model".to_owned(),
                args: vec!["openai/gpt 4".to_owned(), "medium".to_owned()]
            })
        );
    }

    #[test]
    fn escaped_quotes_parse_inside_quotes() {
        let parsed = parse_line(r#"/echo "say \"hello\"""#).unwrap();

        assert_eq!(
            parsed,
            ParseOutcome::Command(CommandInvocation {
                name: "echo".to_owned(),
                args: vec![r#"say "hello""#.to_owned()]
            })
        );
    }

    #[test]
    fn unterminated_quote_is_rejected() {
        assert!(matches!(
            parse_line(r#"/model "openrouter/model"#),
            Err(CommandError::UnterminatedQuote)
        ));
    }

    #[test]
    fn uppercase_command_is_unknown() {
        assert!(matches!(
            parse_line("/History"),
            Err(CommandError::UnknownCommand { name }) if name == "History"
        ));
    }

    #[test]
    fn duplicate_registration_fails() {
        #[derive(Default)]
        struct Context;

        fn execute<'a>(_context: &'a mut Context, _args: Vec<String>) -> CommandFuture<'a> {
            Box::pin(async { Ok(CommandControl::Continue) })
        }

        let command = Command {
            name: "test",
            usage: "/test",
            summary: "test",
            execute,
        };
        let mut registry = CommandRegistry::<Context>::new();
        registry.register(command).unwrap();

        assert!(matches!(
            registry.register(command),
            Err(CommandError::DuplicateCommand { name }) if name == "test"
        ));
    }

    #[test]
    fn search_empty_query_returns_all_commands() {
        let registry = test_registry();
        let matches = registry.search("", 8);

        assert_eq!(
            matches
                .iter()
                .map(|entry| entry.metadata.name)
                .collect::<Vec<_>>(),
            vec!["clear", "history", "model", "resume"]
        );
    }

    #[test]
    fn search_ranks_prefix_before_subsequence() {
        let registry = registry_with([("haiku", "Write haiku"), ("history", "List sessions")]);
        let matches = registry.search("hi", 8);

        assert_eq!(matches[0].metadata.name, "history");
        assert!(matches.iter().any(|entry| entry.metadata.name == "haiku"));
    }

    #[test]
    fn search_matches_fuzzy_subsequence() {
        let registry = test_registry();
        let matches = registry.search("hi", 8);

        assert_eq!(
            matches
                .iter()
                .map(|entry| entry.metadata.name)
                .collect::<Vec<_>>(),
            vec!["history"]
        );
    }

    #[test]
    fn search_is_case_sensitive() {
        let registry = test_registry();

        assert!(registry.search("History", 8).is_empty());
    }

    fn test_registry() -> CommandRegistry<()> {
        registry_with([
            ("clear", "Clear terminal"),
            ("history", "List sessions"),
            ("model", "Set model"),
            ("resume", "Resume session"),
        ])
    }

    fn registry_with<const N: usize>(
        entries: [(&'static str, &'static str); N],
    ) -> CommandRegistry<()> {
        fn execute<'a>(_context: &'a mut (), _args: Vec<String>) -> CommandFuture<'a> {
            Box::pin(async { Ok(CommandControl::Continue) })
        }

        let mut registry = CommandRegistry::new();
        for (name, summary) in entries {
            registry
                .register(Command {
                    name,
                    usage: name,
                    summary,
                    execute,
                })
                .unwrap();
        }
        registry
    }
}
