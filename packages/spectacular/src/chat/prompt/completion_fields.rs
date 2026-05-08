/// Finds the required or invalid field that should block command submission.
fn command_field_needing_attention(
    buffer: &str,
    completions: &PromptCompletionCatalog<'_>,
) -> Option<CompletionFieldSpec> {
    let (_, subcommand, args, fields) = command_fields(buffer, completions)?;
    let subcommand = subcommand.as_deref()?;
    command_validation_state(&args, fields, subcommand, completions).next_field()
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CommandValidationState {
    missing: Vec<CompletionFieldSpec>,
    invalid: Option<InvalidChoiceField>,
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
struct InvalidChoiceField {
    field: CompletionFieldSpec,
    value: String,
    allowed: Vec<String>,
}

type CommandFields = (String, Option<String>, Vec<String>, &'static [CompletionFieldSpec]);

/// Summarizes missing required fields and invalid closed-choice values for a command.
fn command_validation_state(
    args: &[String],
    fields: &'static [CompletionFieldSpec],
    subcommand: &str,
    completions: &PromptCompletionCatalog<'_>,
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
        invalid: invalid_choice_field(&pairs, fields, subcommand, completions),
    }
}

/// Finds a completed command path and its declared field metadata.
fn command_fields(
    buffer: &str,
    completions: &PromptCompletionCatalog<'_>,
) -> Option<CommandFields> {
    let (command, args) = parsed_command_line(buffer)?;
    let Some(subcommand) = args.first() else {
        let spec = completions.spec(&command)?;
        return Some((
            command,
            None,
            args,
            spec.subcommands.first().map_or(&[], |spec| spec.fields),
        ));
    };
    let subcommand_spec = completions.subcommand(&command, subcommand)?;

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
    completions: &PromptCompletionCatalog<'_>,
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

    completions.field(&command, &subcommand, &field_query)
}

/// Finds the first closed-choice field whose value is outside the allowed set.
fn invalid_choice_field(
    pairs: &[(String, String)],
    fields: &'static [CompletionFieldSpec],
    subcommand: &str,
    completions: &PromptCompletionCatalog<'_>,
) -> Option<InvalidChoiceField> {
    pairs.iter().find_map(|(name, value)| {
        if value.trim().is_empty() {
            return None;
        }

        let field = find_field(fields, name)?;
        if !field.validates_one_of_values() {
            return None;
        }

        match completions.validate_choice(field, value, subcommand, pairs) {
            Ok(ChoiceValidation::Valid) => None,
            Ok(ChoiceValidation::Invalid(allowed)) => Some(InvalidChoiceField {
                field,
                value: value.clone(),
                allowed,
            }),
            Err(error) => Some(InvalidChoiceField {
                field,
                value: value.clone(),
                allowed: vec![format!("{} values unavailable: {error}", field.name)],
            }),
        }
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


/// Replaces the active completion token and optionally inserts trailing spacing.
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
