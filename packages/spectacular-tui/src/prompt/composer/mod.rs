mod catalog;
mod context;
mod edit;

use crate::metadata::{CommandDescriptor, CommandFieldDescriptor};
use crate::session::PromptState;
use catalog::{
    active_field_spec, command_field, command_field_needing_attention, command_fields,
    command_subcommand, command_validation_state, field_guidance_line, resolve_values,
    ResolvedValues,
};
use context::{
    command_name_query as context_command_name_query, completion_context, context_key, named_pairs,
    used_fields, CompletionTarget,
};
use edit::{complete_suggestion, guide_next_field};
use spectacular_commands::fuzzy_filter;

const MAX_SUGGESTIONS: usize = 8;

/// A selectable or informational command-composer completion row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandSuggestion {
    pub replacement: String,
    pub label: String,
    pub summary: String,
    pub append_space: bool,
    pub kind: CommandSuggestionKind,
}

/// Identifies which command-composer element a suggestion completes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandSuggestionKind {
    Command,
    Subcommand,
    Field,
    Value,
    Info,
}

/// One command-composer guidance row rendered below the prompt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandGuidanceLine {
    Missing(Vec<String>),
    Detail(String),
    Info(String),
}

struct ValueSuggestionRequest<'a> {
    subcommand: &'a str,
    field: &'a str,
    query: &'a str,
    args: &'a [(String, String)],
    descriptor: &'a CommandFieldDescriptor,
}

/// Builds command-composer suggestions for command, subcommand, field, or value tokens.
pub fn command_suggestions<'a>(
    prompt: &PromptState,
    commands: &'a [CommandDescriptor],
) -> Vec<CommandSuggestion> {
    if command_completion_context_key(prompt) == prompt.dismissed_completion {
        return Vec::new();
    }

    command_suggestions_for_prompt(prompt, commands)
}

fn command_suggestions_for_prompt(
    prompt: &PromptState,
    commands: &[CommandDescriptor],
) -> Vec<CommandSuggestion> {
    let text = prompt.text();
    let Some(context) = completion_context(&text, prompt.cursor) else {
        return Vec::new();
    };

    match context.target {
        CompletionTarget::Command => command_name_suggestions(commands, context.query),
        CompletionTarget::Subcommand { command } => commands
            .iter()
            .find(|descriptor| descriptor.name == command)
            .map(|descriptor| subcommand_suggestions(descriptor, context.query))
            .unwrap_or_default(),
        CompletionTarget::Field {
            command,
            subcommand,
            used_fields,
        } => command_subcommand(commands, &command, &subcommand)
            .map(|subcommand| field_suggestions(&subcommand.fields, &used_fields, context.query))
            .unwrap_or_default(),
        CompletionTarget::Value {
            command,
            subcommand,
            field,
            field_query,
            value_query,
            args,
        } => command_field(commands, &command, &subcommand, &field_query)
            .map(|descriptor| {
                value_suggestions(ValueSuggestionRequest {
                    subcommand: &subcommand,
                    field: &field,
                    query: &value_query,
                    args: &args,
                    descriptor,
                })
            })
            .unwrap_or_default(),
    }
}

/// Returns the stable context key used to dismiss the active completion picker.
pub fn command_completion_context_key(prompt: &PromptState) -> Option<String> {
    let text = prompt.text();
    let context = completion_context(&text, prompt.cursor)?;
    Some(context_key(&context))
}

/// Hides the currently visible suggestions until the completion context changes.
pub fn dismiss_command_suggestions(
    prompt: &mut PromptState,
    commands: &[CommandDescriptor],
) -> bool {
    let key = command_completion_context_key(prompt);
    if key.is_none() || key == prompt.dismissed_completion {
        return false;
    }

    if command_suggestions_for_prompt(prompt, commands).is_empty() {
        return false;
    }

    prompt.dismissed_completion = key;
    prompt.selected_completion = 0;
    true
}

/// Returns true when the prompt is composing a first-line slash command.
pub fn has_command_context(prompt: &PromptState) -> bool {
    prompt
        .text()
        .lines()
        .next()
        .is_some_and(|line| line.starts_with('/'))
}

/// Builds compact command-composer guidance for the active slash command.
pub fn command_guidance(
    prompt: &PromptState,
    commands: &[CommandDescriptor],
) -> Vec<CommandGuidanceLine> {
    let text = prompt.text();
    let Some((_, subcommand, args, fields)) = command_fields(&text, commands) else {
        return Vec::new();
    };
    if subcommand.is_none() {
        return Vec::new();
    }

    let pairs = named_pairs(&args[1..]);
    let validation =
        command_validation_state(&pairs, fields, subcommand.as_deref().unwrap_or_default());

    let mut lines = Vec::new();
    if !validation.missing.is_empty() {
        lines.push(CommandGuidanceLine::Missing(
            validation
                .missing
                .iter()
                .map(|field| field.name.to_owned())
                .collect(),
        ));
    }

    if let Some(invalid) = &validation.invalid {
        lines.push(CommandGuidanceLine::Info(format!(
            "invalid: {}:{}",
            invalid.field.name, invalid.value
        )));
        lines.push(CommandGuidanceLine::Detail(format!(
            "allowed: {}",
            invalid.allowed.join(", ")
        )));
    }

    if let Some(field) = active_field_spec(&text, prompt.cursor, commands) {
        lines.push(CommandGuidanceLine::Detail(field_guidance_line(field)));
        return lines;
    }

    if let Some(field) = validation.next_field() {
        lines.push(CommandGuidanceLine::Detail(field_guidance_line(field)));
        return lines;
    }

    let used = used_fields(&args[1..]);
    let optional = fields
        .iter()
        .filter(|field| !field.required)
        .filter(|field| !used.iter().any(|used| used == &field.name))
        .map(|field| field.name.as_str())
        .collect::<Vec<_>>();
    if !optional.is_empty() {
        return vec![CommandGuidanceLine::Detail(format!(
            "optional: {}",
            optional.join(", ")
        ))];
    }

    vec![CommandGuidanceLine::Detail(
        "ready: Enter to run".to_owned(),
    )]
}

/// Accepts the currently selected command-composer suggestion into the prompt.
pub fn accept_command_suggestion(
    prompt: &mut PromptState,
    suggestion: &CommandSuggestion,
    commands: &[CommandDescriptor],
) {
    if suggestion.kind == CommandSuggestionKind::Info {
        return;
    }

    let text = prompt.text();
    let Some(context) = completion_context(&text, prompt.cursor) else {
        return;
    };
    let range = context.token_start..context.token_end;
    let mut text = text;
    complete_suggestion(&mut text, &mut prompt.cursor, range, suggestion);
    prompt.replace_text_as_edit(text);

    if matches!(
        suggestion.kind,
        CommandSuggestionKind::Subcommand | CommandSuggestionKind::Value
    ) {
        guide_command_field(prompt, commands);
    }
}

/// Moves the command composer to the next required or invalid field.
pub fn guide_command_field(prompt: &mut PromptState, commands: &[CommandDescriptor]) -> bool {
    let text = prompt.text();
    let Some(field) = command_field_needing_attention(&text, commands) else {
        return false;
    };

    let mut text = text;
    guide_next_field(&mut text, &mut prompt.cursor, &field.name);
    prompt.replace_text_as_edit(text);
    true
}

/// Returns the byte-0 slash command query while editing the leading command token.
pub(crate) fn command_name_query(text: &str, cursor: usize) -> Option<&str> {
    context_command_name_query(text, cursor)
}

fn command_name_suggestions(commands: &[CommandDescriptor], query: &str) -> Vec<CommandSuggestion> {
    let names = commands
        .iter()
        .map(|command| command.name.as_str())
        .collect::<Vec<_>>();
    fuzzy_filter(names, query, MAX_SUGGESTIONS)
        .into_iter()
        .filter_map(|name| commands.iter().find(|command| command.name == name))
        .map(|command| CommandSuggestion {
            replacement: command.name.clone(),
            label: format!("/{}", command.name),
            summary: command.summary.clone(),
            append_space: true,
            kind: CommandSuggestionKind::Command,
        })
        .collect()
}

fn subcommand_suggestions(command: &CommandDescriptor, query: &str) -> Vec<CommandSuggestion> {
    let names = command
        .subcommands
        .iter()
        .map(|subcommand| subcommand.name.as_str())
        .collect::<Vec<_>>();
    fuzzy_filter(names, query, MAX_SUGGESTIONS)
        .into_iter()
        .filter_map(|name| {
            command
                .subcommands
                .iter()
                .find(|subcommand| subcommand.name == name)
        })
        .map(|subcommand| CommandSuggestion {
            replacement: subcommand.name.clone(),
            label: subcommand.name.clone(),
            summary: subcommand.summary.clone(),
            append_space: true,
            kind: CommandSuggestionKind::Subcommand,
        })
        .collect()
}

fn field_suggestions(
    fields: &[CommandFieldDescriptor],
    used_fields: &[String],
    query: &str,
) -> Vec<CommandSuggestion> {
    let names = fields
        .iter()
        .filter(|field| !used_fields.iter().any(|used| used == &field.name))
        .filter(|field| field.required || !query.is_empty())
        .map(|field| field.name.as_str())
        .collect::<Vec<_>>();

    fuzzy_filter(names, query, MAX_SUGGESTIONS)
        .into_iter()
        .map(|name| {
            let field = fields.iter().find(|field| field.name == name);
            CommandSuggestion {
                replacement: format!("{name}:"),
                label: format!("{name}:"),
                summary: field
                    .map(field_guidance_line)
                    .unwrap_or_else(|| "field".to_owned()),
                append_space: false,
                kind: CommandSuggestionKind::Field,
            }
        })
        .collect()
}

fn value_suggestions(request: ValueSuggestionRequest<'_>) -> Vec<CommandSuggestion> {
    let values = match resolve_values(request.descriptor, request.subcommand, request.args) {
        ResolvedValues::Values(values) => values,
        ResolvedValues::Unavailable(message) => {
            return vec![CommandSuggestion {
                replacement: String::new(),
                label: format!("[{} values unavailable]", request.field),
                summary: message,
                append_space: false,
                kind: CommandSuggestionKind::Info,
            }];
        }
    };
    let (matches, total) = fuzzy_limited_matches(values, request.query, MAX_SUGGESTIONS);

    let mut suggestions = matches
        .into_iter()
        .map(|value| CommandSuggestion {
            replacement: format!("{}:{value}", request.field),
            label: value,
            summary: format!("{} value", request.field),
            append_space: true,
            kind: CommandSuggestionKind::Value,
        })
        .collect::<Vec<_>>();

    let remaining = total.saturating_sub(suggestions.len());
    if remaining > 0 {
        suggestions.push(CommandSuggestion {
            replacement: String::new(),
            label: format!("[more {remaining} items...]"),
            summary: String::new(),
            append_space: false,
            kind: CommandSuggestionKind::Info,
        });
    }

    suggestions
}

fn fuzzy_limited_matches(
    candidates: Vec<String>,
    query: &str,
    limit: usize,
) -> (Vec<String>, usize) {
    let mut matches = candidates
        .iter()
        .filter_map(|candidate| {
            spectacular_commands::fuzzy_rank(candidate, query).map(|rank| (rank, candidate.clone()))
        })
        .collect::<Vec<_>>();
    matches.sort_by(|(left_rank, left), (right_rank, right)| {
        left_rank.cmp(right_rank).then_with(|| left.cmp(right))
    });
    let total = matches.len();
    let values = matches
        .into_iter()
        .take(limit)
        .map(|(_, candidate)| candidate)
        .collect();

    (values, total)
}
