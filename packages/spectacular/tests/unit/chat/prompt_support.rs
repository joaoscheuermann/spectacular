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

    fn test_suggestion(replacement: &str) -> PromptSuggestion {
        PromptSuggestion {
            replacement: replacement.to_owned(),
            label: replacement.to_owned(),
            summary: String::new(),
            append_space: true,
            kind: PromptSuggestionKind::Command,
        }
    }

    fn test_completion_catalog() -> PromptCompletionCatalog {
        const MODEL_ADD_FIELDS: &[CompletionFieldSpec] = &[
            CompletionFieldSpec {
                name: "provider",
                summary: "configured provider name",
                required: true,
                value_source: CompletionValueSource::Dynamic(
                    crate::chat::commands::SOURCE_PROVIDERS,
                ),
            },
            CompletionFieldSpec {
                name: "id",
                summary: "model ID from the selected provider",
                required: true,
                value_source: CompletionValueSource::Dynamic(
                    crate::chat::commands::SOURCE_MODEL_IDS,
                ),
            },
            CompletionFieldSpec {
                name: "reasoning",
                summary: "reasoning level",
                required: true,
                value_source: CompletionValueSource::Static(&[
                    "none", "minimal", "low", "medium", "high", "xhigh",
                ]),
            },
            CompletionFieldSpec {
                name: "name",
                summary: "optional saved model name",
                required: false,
                value_source: CompletionValueSource::Static(&[]),
            },
        ];
        const MODEL_SUBCOMMANDS: &[CompletionSubcommandSpec] = &[
            CompletionSubcommandSpec {
                name: "add",
                summary: "Add model",
                fields: MODEL_ADD_FIELDS,
            },
            CompletionSubcommandSpec {
                name: "edit",
                summary: "Edit model",
                fields: MODEL_ADD_FIELDS,
            },
            CompletionSubcommandSpec {
                name: "remove",
                summary: "Remove model",
                fields: MODEL_ADD_FIELDS,
            },
        ];
        const SPECS: &[CompletionCommandSpec] = &[CompletionCommandSpec {
            name: "model",
            subcommands: MODEL_SUBCOMMANDS,
        }];

        let mut sources = BTreeMap::new();
        sources.insert(
            crate::chat::commands::SOURCE_PROVIDERS.to_owned(),
            vec!["openrouter".to_owned(), "work".to_owned()],
        );
        sources.insert(
            crate::chat::commands::SOURCE_MODEL_IDS.to_owned(),
            vec!["openai/gpt-5.5".to_owned(), "google/gemini".to_owned()],
        );
        sources.insert(
            format!("{}:work", crate::chat::commands::SOURCE_MODEL_IDS),
            vec!["google/gemini".to_owned()],
        );
        sources.insert(
            format!("{}:openrouter", crate::chat::commands::SOURCE_MODEL_IDS),
            vec!["openai/gpt-5.5".to_owned()],
        );
        sources.insert(
            format!("{}:busy", crate::chat::commands::SOURCE_MODEL_IDS),
            (0..10)
                .map(|index| format!("provider/model-{index}"))
                .collect(),
        );

        PromptCompletionCatalog::new(SPECS, sources)
    }

    fn test_registry() -> CommandRegistry<()> {
        fn execute<'a>(_context: &'a mut (), _args: Vec<String>) -> CommandFuture<'a> {
            Box::pin(async { Ok(CommandControl::Continue) })
        }

        let mut registry = CommandRegistry::new();
        for name in ["clear", "history", "resume"] {
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
