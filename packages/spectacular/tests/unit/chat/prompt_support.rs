/// Creates prompt state with the cursor at the end of the provided buffer.
fn state_with(value: &str) -> PromptState {
    PromptState {
        buffer: value.to_owned(),
        cursor: value.len(),
        selection_anchor: None,
        selected: 0,
        preferred_column: None,
        kill_buffer: String::new(),
        dismissed_completion: None,
    }
}

/// Creates a selectable command suggestion for completion insertion tests.
fn test_suggestion(replacement: &str) -> PromptSuggestion {
    PromptSuggestion {
        replacement: replacement.to_owned(),
        label: replacement.to_owned(),
        summary: String::new(),
        append_space: true,
        kind: PromptSuggestionKind::Command,
    }
}

/// Creates the completion catalog used by prompt support tests.
fn test_completion_catalog(model: &ChatModel) -> PromptCompletionCatalog<'_> {
    PromptCompletionCatalog::new(TEST_SPECS, model)
}

const TEST_SPECS: &[CompletionCommandSpec] = &[CompletionCommandSpec {
    name: "test",
    subcommands: TEST_SUBCOMMANDS,
}];

const TEST_ADD_FIELDS: &[CompletionFieldSpec] = &[
    CompletionFieldSpec {
        name: "provider",
        summary: "configured provider name",
        required: true,
        values: test_provider_values,
        validation: CompletionValueValidation::None,
    },
    CompletionFieldSpec {
        name: "id",
        summary: "model ID from the selected provider",
        required: true,
        values: test_model_id_values,
        validation: CompletionValueValidation::None,
    },
    CompletionFieldSpec {
        name: "reasoning",
        summary: "reasoning level",
        required: true,
        values: test_reasoning_values,
        validation: CompletionValueValidation::OneOfValues,
    },
    CompletionFieldSpec {
        name: "name",
        summary: "optional saved model name",
        required: false,
        values: test_no_values,
        validation: CompletionValueValidation::None,
    },
];

const TEST_SUBCOMMANDS: &[CompletionSubcommandSpec] = &[
    CompletionSubcommandSpec {
        name: "add",
        summary: "Add test model",
        fields: TEST_ADD_FIELDS,
    },
    CompletionSubcommandSpec {
        name: "edit",
        summary: "Edit test model",
        fields: TEST_ADD_FIELDS,
    },
    CompletionSubcommandSpec {
        name: "remove",
        summary: "Remove test model",
        fields: TEST_ADD_FIELDS,
    },
];

/// Returns provider values used by prompt completion tests.
fn test_provider_values(
    _: &ChatCompletionContext<'_>,
) -> Result<Vec<String>, crate::chat::ChatError> {
    Ok(vec![
        "openrouter".to_owned(),
        "work".to_owned(),
        "busy".to_owned(),
    ])
}

/// Returns model id values scoped to the provider already typed in tests.
fn test_model_id_values(
    ctx: &ChatCompletionContext<'_>,
) -> Result<Vec<String>, crate::chat::ChatError> {
    match ctx.args.get("provider") {
        Some("work") => Ok(vec!["google/gemini".to_owned()]),
        Some("openrouter") => Ok(vec!["openai/gpt-5.5".to_owned()]),
        Some("busy") => Ok((0..10)
            .map(|index| format!("provider/model-{index}"))
            .collect()),
        _ => Ok(vec![
            "openai/gpt-5.5".to_owned(),
            "google/gemini".to_owned(),
        ]),
    }
}

/// Returns canonical reasoning values used by prompt validation tests.
fn test_reasoning_values(
    _: &ChatCompletionContext<'_>,
) -> Result<Vec<String>, crate::chat::ChatError> {
    Ok(spectacular_config::ReasoningLevel::ALL
        .into_iter()
        .map(|value| value.as_str().to_owned())
        .collect())
}

/// Returns no values for free-form test fields.
fn test_no_values(_: &ChatCompletionContext<'_>) -> Result<Vec<String>, crate::chat::ChatError> {
    Ok(Vec::new())
}

/// Creates a minimal command registry used by prompt completion and editing tests.
fn test_registry() -> CommandRegistry<()> {
    /// Executes a no-op command for registry-only prompt tests.
    fn execute<'a>(_context: &'a mut (), _args: Vec<String>) -> CommandFuture<'a> {
        Box::pin(async { Ok(CommandControl::Continue) })
    }

    let mut registry = CommandRegistry::new();
    for name in ["clear", "history", "resume", "test"] {
        registry
            .register(Command {
                name,
                usage: name,
                summary: name,
                execute,
            })
            .unwrap();
    }
    registry
}

/// Creates a chat model fixture for prompt completion resolvers.
fn test_model() -> crate::chat::model::ChatModel {
    let session = crate::chat::session::SessionManager::new_in(temp_session_dir("prompt"))
        .expect("session manager should be created");
    let mut model = ChatModel::new(session, crate::chat::RuntimeSelection::setup());
    model.start_new_session().unwrap();
    model
}

/// Returns a unique temporary session directory for prompt tests.
fn temp_session_dir(name: &str) -> std::path::PathBuf {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    std::env::temp_dir().join(format!("spectacular-chat-prompt-{name}-{suffix}"))
}

/// Converts raw pasted text into key events for unbracketed paste tests.
fn unbracketed_paste_keys(value: &str) -> Vec<KeyEvent> {
    let mut keys = Vec::new();
    let mut chars = value.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                keys.push(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
            }
            '\n' => keys.push(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            character => keys.push(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE)),
        }
    }
    keys
}
