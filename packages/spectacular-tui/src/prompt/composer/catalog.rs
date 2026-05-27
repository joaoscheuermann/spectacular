use super::context::{completion_context, named_pairs, parsed_command_line, CompletionContext};
use crate::metadata::{
    CachedModelValues, CommandDescriptor, CommandFieldDescriptor, CommandSubcommandDescriptor,
    CommandValueValidation, CompletionValues,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CommandValidationState<'a> {
    pub missing: Vec<&'a CommandFieldDescriptor>,
    pub invalid: Option<InvalidChoiceField<'a>>,
}

impl<'a> CommandValidationState<'a> {
    /// Returns the invalid field or first missing field that should receive focus.
    pub(super) fn next_field(&self) -> Option<&'a CommandFieldDescriptor> {
        if let Some(invalid) = &self.invalid {
            return Some(invalid.field);
        }

        self.missing.first().copied()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct InvalidChoiceField<'a> {
    pub field: &'a CommandFieldDescriptor,
    pub value: String,
    pub allowed: Vec<String>,
}

pub(super) enum ResolvedValues {
    Values(Vec<String>),
    Unavailable(String),
}

pub(super) type CommandFields<'a> = (
    String,
    Option<String>,
    Vec<String>,
    &'a [CommandFieldDescriptor],
);

pub(super) fn command_field_needing_attention<'a>(
    text: &str,
    commands: &'a [CommandDescriptor],
) -> Option<&'a CommandFieldDescriptor> {
    let (_, subcommand, args, fields) = command_fields(text, commands)?;
    let subcommand = subcommand.as_deref()?;
    command_validation_state(&named_pairs(&args[1..]), fields, subcommand).next_field()
}

pub(super) fn command_fields<'a>(
    text: &str,
    commands: &'a [CommandDescriptor],
) -> Option<CommandFields<'a>> {
    let (command, args) = parsed_command_line(text)?;
    let command_descriptor = commands
        .iter()
        .find(|descriptor| descriptor.name == command)?;
    let Some(subcommand) = args.first() else {
        return Some((
            command,
            None,
            args,
            command_descriptor
                .subcommands
                .first()
                .map_or(&[], |subcommand| subcommand.fields.as_slice()),
        ));
    };
    let subcommand_descriptor = command_descriptor
        .subcommands
        .iter()
        .find(|descriptor| descriptor.name == *subcommand)?;

    Some((
        command,
        Some(subcommand.clone()),
        args,
        &subcommand_descriptor.fields,
    ))
}

pub(super) fn command_validation_state<'a>(
    pairs: &[(String, String)],
    fields: &'a [CommandFieldDescriptor],
    subcommand: &str,
) -> CommandValidationState<'a> {
    let missing = fields
        .iter()
        .filter(|field| field.required)
        .filter(|field| {
            !pairs
                .iter()
                .any(|(name, value)| name == &field.name && !value.trim().is_empty())
        })
        .collect::<Vec<_>>();

    CommandValidationState {
        missing,
        invalid: invalid_choice_field(pairs, fields, subcommand),
    }
}

pub(super) fn active_field_spec<'a>(
    text: &str,
    cursor: usize,
    commands: &'a [CommandDescriptor],
) -> Option<&'a CommandFieldDescriptor> {
    let CompletionContext {
        target:
            super::context::CompletionTarget::Value {
                command,
                subcommand,
                field_query,
                ..
            },
        ..
    } = completion_context(text, cursor)?
    else {
        return None;
    };

    command_field(commands, &command, &subcommand, &field_query)
}

pub(super) fn command_subcommand<'a>(
    commands: &'a [CommandDescriptor],
    command: &str,
    subcommand: &str,
) -> Option<&'a CommandSubcommandDescriptor> {
    commands
        .iter()
        .find(|descriptor| descriptor.name == command)?
        .subcommands
        .iter()
        .find(|descriptor| descriptor.name == subcommand)
}

pub(super) fn command_field<'a>(
    commands: &'a [CommandDescriptor],
    command: &str,
    subcommand: &str,
    field: &str,
) -> Option<&'a CommandFieldDescriptor> {
    command_subcommand(commands, command, subcommand)?
        .fields
        .iter()
        .find(|descriptor| descriptor.name == field)
}

pub(super) fn resolve_values(
    field: &CommandFieldDescriptor,
    subcommand: &str,
    pairs: &[(String, String)],
) -> ResolvedValues {
    match &field.values {
        CompletionValues::Static(values) => ResolvedValues::Values(values.clone()),
        CompletionValues::ConfiguredProviders(values) => ResolvedValues::Values(values.clone()),
        CompletionValues::SavedModels(values) => ResolvedValues::Values(values.clone()),
        CompletionValues::CachedModelIds(values) => {
            let provider = typed_or_inferred_provider(subcommand, pairs, field, values);
            ResolvedValues::Values(cached_model_ids(values, provider.as_deref()))
        }
        CompletionValues::Unavailable(message) => ResolvedValues::Unavailable(message.clone()),
        CompletionValues::None => ResolvedValues::Values(Vec::new()),
    }
}

pub(super) fn field_guidance_line(field: &CommandFieldDescriptor) -> String {
    let status = if field.required {
        "required"
    } else {
        "optional"
    };

    format!("{} - {}, {status}", field.name, field.summary)
}

fn invalid_choice_field<'a>(
    pairs: &[(String, String)],
    fields: &'a [CommandFieldDescriptor],
    subcommand: &str,
) -> Option<InvalidChoiceField<'a>> {
    pairs.iter().find_map(|(name, value)| {
        if value.trim().is_empty() {
            return None;
        }

        let field = fields.iter().find(|field| field.name == *name)?;
        if field.validation != CommandValueValidation::OneOfValues {
            return None;
        }

        let ResolvedValues::Values(allowed) = resolve_values(field, subcommand, pairs) else {
            return None;
        };
        if allowed.is_empty() || allowed.iter().any(|allowed| allowed == value) {
            return None;
        }

        Some(InvalidChoiceField {
            field,
            value: value.clone(),
            allowed,
        })
    })
}

fn typed_or_inferred_provider(
    subcommand: &str,
    pairs: &[(String, String)],
    field: &CommandFieldDescriptor,
    models: &CachedModelValues,
) -> Option<String> {
    if let Some(provider) = pair_value(pairs, "provider") {
        return Some(provider.to_owned());
    }

    if subcommand != "edit" || field.name != "id" {
        return None;
    }

    let model_name = pair_value(pairs, "name")?;
    models.saved_model_providers.get(model_name).cloned()
}

fn cached_model_ids(models: &CachedModelValues, provider: Option<&str>) -> Vec<String> {
    let Some(provider) = provider else {
        let mut values = models
            .model_ids_by_provider
            .values()
            .flat_map(|models| models.iter().cloned())
            .collect::<Vec<_>>();
        values.sort();
        values.dedup();
        return values;
    };

    models
        .model_ids_by_provider
        .get(provider)
        .cloned()
        .unwrap_or_default()
}

fn pair_value<'a>(pairs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    pairs
        .iter()
        .find(|(field, _)| field == name)
        .map(|(_, value)| value.as_str())
        .filter(|value| !value.trim().is_empty())
}
