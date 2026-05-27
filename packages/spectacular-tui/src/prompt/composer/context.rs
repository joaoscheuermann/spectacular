use spectacular_commands::{parse_line, ParseOutcome};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CompletionContext<'a> {
    pub token_start: usize,
    pub token_end: usize,
    pub query: &'a str,
    pub target: CompletionTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CompletionTarget {
    Command,
    Subcommand {
        command: String,
    },
    Field {
        command: String,
        subcommand: String,
        used_fields: Vec<String>,
    },
    Value {
        command: String,
        subcommand: String,
        field: String,
        field_query: String,
        value_query: String,
        args: Vec<(String, String)>,
    },
}

pub(super) fn command_name_query(text: &str, cursor: usize) -> Option<&str> {
    let context = completion_context(text, cursor)?;
    matches!(context.target, CompletionTarget::Command).then_some(context.query)
}

pub(super) fn completion_context(text: &str, cursor: usize) -> Option<CompletionContext<'_>> {
    if !text.starts_with('/') || cursor == 0 || cursor > text.len() {
        return None;
    }

    if line_start(text, cursor) != 0 {
        return None;
    }

    let token_start = token_start(text, cursor);
    let token_end = token_end(text, cursor);
    let query = &text[token_start..cursor];
    let tokens = command_tokens_before(text, token_start);

    if tokens.is_empty() {
        return Some(CompletionContext {
            token_start,
            token_end,
            query,
            target: CompletionTarget::Command,
        });
    }

    if tokens.len() == 1 {
        return Some(CompletionContext {
            token_start,
            token_end,
            query,
            target: CompletionTarget::Subcommand {
                command: tokens[0].clone(),
            },
        });
    }

    let command = tokens[0].clone();
    let subcommand = tokens[1].clone();
    if let Some((field, value_query)) = query.split_once(':') {
        return Some(CompletionContext {
            token_start,
            token_end,
            query,
            target: CompletionTarget::Value {
                command,
                subcommand,
                field: field.to_owned(),
                field_query: field.to_owned(),
                value_query: value_query.to_owned(),
                args: named_pairs(&tokens[2..]),
            },
        });
    }

    Some(CompletionContext {
        token_start,
        token_end,
        query,
        target: CompletionTarget::Field {
            command,
            subcommand,
            used_fields: used_fields(&tokens[2..]),
        },
    })
}

pub(super) fn context_key(context: &CompletionContext<'_>) -> String {
    format!(
        "{}:{}:{}",
        context.token_start, context.token_end, context.query
    )
}

pub(super) fn parsed_command_line(text: &str) -> Option<(String, Vec<String>)> {
    let line = text.lines().next().unwrap_or_default();
    match parse_line(line).ok()? {
        ParseOutcome::Command(invocation) => Some((invocation.name, invocation.args)),
        ParseOutcome::NotCommand => None,
    }
}

pub(super) fn used_fields(tokens: &[String]) -> Vec<String> {
    tokens
        .iter()
        .filter_map(|token| token.split_once(':').map(|(field, _)| field.to_owned()))
        .collect()
}

pub(super) fn named_pairs(tokens: &[String]) -> Vec<(String, String)> {
    tokens
        .iter()
        .filter_map(|token| {
            let (field, value) = token.split_once(':')?;
            Some((field.to_owned(), value.to_owned()))
        })
        .collect()
}

pub(super) fn line_start(text: &str, cursor: usize) -> usize {
    text[..cursor]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0)
}

pub(super) fn line_end(text: &str, cursor: usize) -> usize {
    text[cursor..]
        .find('\n')
        .map(|index| cursor + index)
        .unwrap_or(text.len())
}

pub(super) fn next_boundary(text: &str, cursor: usize) -> usize {
    text[cursor..]
        .char_indices()
        .nth(1)
        .map(|(index, _)| cursor + index)
        .unwrap_or(text.len())
}

fn token_start(text: &str, cursor: usize) -> usize {
    let line_start = line_start(text, cursor);
    text[line_start..cursor]
        .char_indices()
        .rev()
        .find(|(_, character)| character.is_whitespace())
        .map(|(index, character)| line_start + index + character.len_utf8())
        .unwrap_or(usize::from(text.starts_with('/')))
}

fn token_end(text: &str, cursor: usize) -> usize {
    let line_end = line_end(text, cursor);
    text[cursor..line_end]
        .char_indices()
        .find(|(_, character)| character.is_whitespace())
        .map(|(index, _)| cursor + index)
        .unwrap_or(line_end)
}

fn command_tokens_before(text: &str, token_start: usize) -> Vec<String> {
    text[1..token_start]
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}
