#[cfg(test)]
/// Extracts the slash-command query under the cursor for command-name tests.
fn suggestion_query(buffer: &str, cursor: usize) -> Option<&str> {
    if !buffer.starts_with('/') || cursor == 0 || cursor > buffer.len() {
        return None;
    }

    let command_prefix = &buffer[..cursor];
    if command_prefix.chars().any(char::is_whitespace) {
        return None;
    }

    Some(&buffer[1..cursor])
}

/// Builds prompt suggestions for the active command, subcommand, field, or value token.
fn prompt_suggestions<C>(
    buffer: &str,
    cursor: usize,
    registry: &CommandRegistry<C>,
    completions: &PromptCompletionCatalog<'_>,
) -> Vec<PromptSuggestion> {
    let Some(context) = completion_context(buffer, cursor) else {
        return Vec::new();
    };

    match context.target {
        CompletionTarget::Command => registry
            .search(context.query, MAX_SUGGESTIONS)
            .into_iter()
            .map(|entry| PromptSuggestion {
                replacement: entry.metadata.name.to_owned(),
                label: format!("/{}", entry.metadata.name),
                summary: entry.metadata.summary.to_owned(),
                append_space: true,
                kind: PromptSuggestionKind::Command,
            })
            .collect(),
        CompletionTarget::Subcommand { command } => completions
            .spec(&command)
            .map(|spec| subcommand_suggestions(spec.subcommands, context.query))
            .unwrap_or_default(),
        CompletionTarget::Field {
            command,
            subcommand,
            used_fields,
        } => completions
            .subcommand(&command, &subcommand)
            .map(|spec| field_suggestions(spec.fields, &used_fields, context.query))
            .unwrap_or_default(),
        CompletionTarget::Value {
            command,
            subcommand,
            field,
            field_query,
            value_query,
            args,
        } => completions
            .field(&command, &subcommand, &field_query)
            .map(|spec| {
                value_suggestions(
                    ValueSuggestionRequest {
                        subcommand: &subcommand,
                        field: &field,
                        query: &value_query,
                        args: &args,
                        spec,
                    },
                    completions,
                )
            })
            .unwrap_or_default(),
    }
}

/// Returns ranked subcommand suggestions for a completion query.
fn subcommand_suggestions(
    specs: &[CompletionSubcommandSpec],
    query: &str,
) -> Vec<PromptSuggestion> {
    let names = specs.iter().map(|spec| spec.name).collect::<Vec<_>>();
    fuzzy_filter(names, query, MAX_SUGGESTIONS)
        .into_iter()
        .filter_map(|name| find_subcommand(specs, name))
        .map(|spec| PromptSuggestion {
            replacement: spec.name.to_owned(),
            label: spec.name.to_owned(),
            summary: spec.summary.to_owned(),
            append_space: true,
            kind: PromptSuggestionKind::Subcommand,
        })
        .collect()
}

/// Returns field-name suggestions excluding fields already used in the command line.
fn field_suggestions(
    specs: &[CompletionFieldSpec],
    used_fields: &[String],
    query: &str,
) -> Vec<PromptSuggestion> {
    let names = specs
        .iter()
        .filter(|spec| !used_fields.iter().any(|field| field == spec.name))
        .filter(|spec| spec.required || !query.is_empty())
        .map(|spec| spec.name)
        .collect::<Vec<_>>();

    fuzzy_filter(names, query, MAX_SUGGESTIONS)
        .into_iter()
        .map(|name| PromptSuggestion {
            replacement: format!("{name}:"),
            label: format!("{name}:"),
            summary: find_field(specs, name)
                .map(field_guidance_line)
                .unwrap_or_else(|| "field".to_owned()),
            append_space: false,
            kind: PromptSuggestionKind::Field,
        })
        .collect()
}

/// Captures all prompt state needed to resolve values for one command field.
struct ValueSuggestionRequest<'a> {
    subcommand: &'a str,
    field: &'a str,
    query: &'a str,
    args: &'a [(String, String)],
    spec: CompletionFieldSpec,
}

/// Returns ranked field-value suggestions or a non-selectable error suggestion.
fn value_suggestions(
    request: ValueSuggestionRequest<'_>,
    completions: &PromptCompletionCatalog<'_>,
) -> Vec<PromptSuggestion> {
    let values = match completions.resolve_values(request.spec, request.subcommand, request.args) {
        Ok(values) => values,
        Err(error) => return unavailable_value_suggestion(request.field, &error.to_string()),
    };
    let (matches, total) = fuzzy_limited_matches(values, request.query, MAX_SUGGESTIONS);

    let mut suggestions = matches
        .into_iter()
        .map(|value| PromptSuggestion {
            replacement: format!("{}:{value}", request.field),
            label: value.to_owned(),
            summary: format!("{} value", request.field),
            append_space: true,
            kind: PromptSuggestionKind::Value,
        })
        .collect::<Vec<_>>();

    let remaining = total.saturating_sub(suggestions.len());
    if remaining > 0 {
        suggestions.push(PromptSuggestion {
            replacement: String::new(),
            label: format!("[more {remaining} items...]"),
            summary: String::new(),
            append_space: false,
            kind: PromptSuggestionKind::Info,
        });
    }

    suggestions
}

/// Returns a non-selectable suggestion explaining that values could not be resolved.
fn unavailable_value_suggestion(field: &str, error: &str) -> Vec<PromptSuggestion> {
    vec![PromptSuggestion {
        replacement: String::new(),
        label: format!("[{field} values unavailable]"),
        summary: error.to_owned(),
        append_space: false,
        kind: PromptSuggestionKind::Info,
    }]
}

/// Applies the shared fuzzy ranking while also reporting the hidden match count.
fn fuzzy_limited_matches(
    candidates: Vec<String>,
    query: &str,
    limit: usize,
) -> (Vec<String>, usize) {
    let mut matches = candidates
        .into_iter()
        .filter_map(|candidate| fuzzy_rank(&candidate, query).map(|rank| (rank, candidate)))
        .collect::<Vec<_>>();

    matches.sort_by(|(left_rank, left), (right_rank, right)| {
        left_rank.cmp(right_rank).then_with(|| left.cmp(right))
    });

    let total = matches.len();
    (
        matches
            .into_iter()
            .take(limit)
            .map(|(_, candidate)| candidate)
            .collect(),
        total,
    )
}

/// Builds compact command-composer guidance for the active slash command.
fn prompt_guidance(
    buffer: &str,
    cursor: usize,
    completions: &PromptCompletionCatalog<'_>,
) -> Vec<PromptGuidanceLine> {
    let Some((_, subcommand, args, fields)) = command_fields(buffer, completions) else {
        return Vec::new();
    };
    if subcommand.is_none() {
        return Vec::new();
    }

    let validation = command_validation_state(
        &args,
        fields,
        subcommand.as_deref().unwrap_or_default(),
        completions,
    );
    let mut lines = Vec::new();
    if !validation.missing.is_empty() {
        lines.push(PromptGuidanceLine::Missing(
            validation
                .missing
                .iter()
                .map(|field| field.name.to_owned())
                .collect(),
        ));
    }

    if let Some(invalid) = &validation.invalid {
        lines.push(PromptGuidanceLine::Info(format!(
            "invalid: {}:{}",
            invalid.field.name, invalid.value
        )));
        lines.push(PromptGuidanceLine::Detail(format!(
            "allowed: {}",
            invalid.allowed.join(", ")
        )));
    }

    if let Some(field) = active_field_spec(buffer, cursor, completions) {
        lines.push(PromptGuidanceLine::Detail(field_guidance_line(field)));
        return lines;
    }

    if let Some(field) = validation.next_field() {
        lines.push(PromptGuidanceLine::Detail(field_guidance_line(field)));
        return lines;
    }

    let used = used_fields(&args[1..]);
    let optional = fields
        .iter()
        .filter(|field| !field.required)
        .filter(|field| !used.iter().any(|used| used == field.name))
        .map(|field| field.name)
        .collect::<Vec<_>>();
    if !optional.is_empty() {
        return vec![PromptGuidanceLine::Detail(format!(
            "optional: {}",
            optional.join(", ")
        ))];
    }

    vec![PromptGuidanceLine::Detail("ready: Enter to run".to_owned())]
}
