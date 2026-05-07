/// Finds the required or invalid field that should block command submission.
fn command_field_needing_attention(
    buffer: &str,
    completions: &PromptCompletionCatalog,
) -> Option<CompletionFieldSpec> {
    let (_, subcommand, args, fields) = command_fields(buffer, completions)?;
    subcommand.as_ref()?;
    command_validation_state(&args, fields).next_field()
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CommandValidationState {
    missing: Vec<CompletionFieldSpec>,
    invalid: Option<InvalidStaticField>,
}

impl CommandValidationState {
    /// Returns the invalid field or first missing field that should receive focus.
    fn next_field(&self) -> Option<CompletionFieldSpec> {
        if let Some(invalid) = &self.invalid {
            return Some(invalid.field);
        }

        self.missing.first().copied()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InvalidStaticField {
    field: CompletionFieldSpec,
    value: String,
    allowed: Vec<&'static str>,
}

type CommandFields = (
    String,
    Option<String>,
    Vec<String>,
    &'static [CompletionFieldSpec],
);

/// Summarizes missing required fields and invalid static choices for a command.
fn command_validation_state(
    args: &[String],
    fields: &'static [CompletionFieldSpec],
) -> CommandValidationState {
    let pairs = named_pairs(&args[1..]);
    let missing = fields
        .iter()
        .copied()
        .filter(|field| field.required)
        .filter(|field| {
            !pairs
                .iter()
                .any(|(name, value)| name == field.name && !value.trim().is_empty())
        })
        .collect::<Vec<_>>();

    CommandValidationState {
        missing,
        invalid: invalid_static_field(&pairs, fields),
    }
}

/// Finds a completed command path and its declared field metadata.
fn command_fields(
    buffer: &str,
    completions: &PromptCompletionCatalog,
) -> Option<CommandFields> {
    let (command, args) = parsed_command_line(buffer)?;
    let spec = completions.spec(&command)?;
    let Some(subcommand) = args.first() else {
        return Some((command, None, args, &[]));
    };
    let subcommand_spec = find_subcommand(spec.subcommands, subcommand)?;

    Some((
        command,
        Some(subcommand.clone()),
        args,
        subcommand_spec.fields,
    ))
}

/// Returns metadata for the field token currently under the cursor.
fn active_field_spec(
    buffer: &str,
    cursor: usize,
    completions: &PromptCompletionCatalog,
) -> Option<CompletionFieldSpec> {
    let CompletionContext {
        target:
            CompletionTarget::Value {
                command,
                subcommand,
                field_query,
                ..
            },
        ..
    } = completion_context(buffer, cursor)?
    else {
        return None;
    };

    completions
        .spec(&command)
        .and_then(|spec| find_subcommand(spec.subcommands, &subcommand))
        .and_then(|spec| find_field(spec.fields, &field_query))
}

/// Finds the first static-choice field whose value is outside the allowed set.
fn invalid_static_field(
    pairs: &[(String, String)],
    fields: &'static [CompletionFieldSpec],
) -> Option<InvalidStaticField> {
    pairs.iter().find_map(|(name, value)| {
        if value.trim().is_empty() {
            return None;
        }

        let field = find_field(fields, name)?;
        let CompletionValueSource::Static(values) = field.value_source else {
            return None;
        };
        if values.is_empty() || values.iter().any(|allowed| allowed == value) {
            return None;
        }

        Some(InvalidStaticField {
            field,
            value: value.clone(),
            allowed: values.to_vec(),
        })
    })
}

/// Formats a single field help line with its required or optional status.
fn field_guidance_line(field: CompletionFieldSpec) -> String {
    let status = if field.required {
        "required"
    } else {
        "optional"
    };

    format!("{} - {}, {status}", field.name, field.summary)
}

/// Parses the first prompt line as a slash command using the shared parser.
fn parsed_command_line(buffer: &str) -> Option<(String, Vec<String>)> {
    let line = buffer.lines().next().unwrap_or_default();
    match parse_line(line).ok()? {
        ParseOutcome::Command(invocation) => Some((invocation.name, invocation.args)),
        ParseOutcome::NotCommand => None,
    }
}

/// Moves the cursor to an existing field token or appends that field at line end.
fn focus_command_field(buffer: &mut String, cursor: &mut usize, field: &str) {
    if let Some(range) = field_token_range(buffer, *cursor, field) {
        *cursor = range.end;
        return;
    }

    append_command_field(buffer, cursor, field);
}

/// Finds the token range for a named field on the active command line.
fn field_token_range(buffer: &str, cursor: usize, field: &str) -> Option<Range<usize>> {
    let start = line_start(buffer, cursor);
    let end = line_end(buffer, cursor);
    let mut token_start = None;

    for (offset, character) in buffer[start..end].char_indices() {
        let index = start + offset;
        if character.is_whitespace() {
            if let Some(token_start) = token_start.take() {
                let token = &buffer[token_start..index];
                if token.starts_with(field) && token.as_bytes().get(field.len()) == Some(&b':') {
                    return Some(token_start..index);
                }
            }
            continue;
        }

        token_start.get_or_insert(index);
    }

    let token_start = token_start?;
    let token = &buffer[token_start..end];
    (token.starts_with(field) && token.as_bytes().get(field.len()) == Some(&b':'))
        .then_some(token_start..end)
}

/// Appends a named field token to the end of the active command line.
fn append_command_field(buffer: &mut String, cursor: &mut usize, field: &str) {
    let end = line_end(buffer, *cursor);
    let needs_space = buffer[..end]
        .chars()
        .last()
        .is_some_and(|character| !character.is_whitespace());
    let insertion = if needs_space {
        format!(" {field}:")
    } else {
        format!("{field}:")
    };

    buffer.insert_str(end, &insertion);
    *cursor = end + insertion.len();
}

fn dynamic_values_for_field<'a>(
    source: &str,
    field: &str,
    args: &'a [(String, String)],
    completions: &'a PromptCompletionCatalog,
) -> Vec<&'a str> {
    if source != crate::chat::commands::SOURCE_MODEL_IDS {
        return completions.source(source);
    }

    let Some(provider) = args
        .iter()
        .find(|(name, _)| name == "provider")
        .map(|(_, value)| value.as_str())
    else {
        return completions.source(source);
    };

    let provider_source = format!("{}:{provider}", crate::chat::commands::SOURCE_MODEL_IDS);
    let provider_values = completions.source(&provider_source);
    if field == "id" && !provider_values.is_empty() {
        return provider_values;
    }

    completions.source(source)
}

fn complete_suggestion(buffer: &mut String, cursor: &mut usize, suggestion: &PromptSuggestion) {
    let Some((token_start, token_end)) =
        completion_context(buffer, *cursor).map(|context| (context.token_start, context.token_end))
    else {
        return;
    };

    buffer.replace_range(token_start..token_end, &suggestion.replacement);

    let insert_at = token_start + suggestion.replacement.len();
    if !suggestion.append_space {
        *cursor = insert_at;
        return;
    }

    if buffer[insert_at..]
        .chars()
        .next()
        .is_some_and(char::is_whitespace)
    {
        *cursor = next_boundary(buffer, insert_at);
        return;
    }

    buffer.insert(insert_at, ' ');
    *cursor = insert_at + 1;
}
