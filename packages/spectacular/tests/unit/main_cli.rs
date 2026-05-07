use super::*;
use clap::CommandFactory;
use spectacular_config::{
    CachedModelMetadata, ModelCache, ModelConfig, ProviderModelCache, TaskAssignments,
};
use std::cell::RefCell;
use std::collections::BTreeMap;

#[test]
fn top_level_help_lists_chat_config_and_plan() {
    let mut command = Cli::command();
    let mut buffer = Vec::new();

    command.write_long_help(&mut buffer).unwrap();
    let help = String::from_utf8(buffer).unwrap();

    assert!(help.contains("chat"));
    assert!(help.contains("config"));
    assert!(help.contains("plan"));
}

#[test]
fn config_help_lists_nested_subcommands() {
    let mut command = Cli::command();
    let config = command
        .find_subcommand_mut("config")
        .expect("config subcommand should exist");
    let mut buffer = Vec::new();

    config.write_long_help(&mut buffer).unwrap();
    let help = String::from_utf8(buffer).unwrap();

    assert!(help.contains("provider"));
    assert!(help.contains("model"));
    assert!(help.contains("task"));
    assert!(!help.contains("--key"));
    assert!(!help.contains("--reasoning"));
}

#[test]
fn config_defaults_to_show_operation() {
    let args = ConfigArgs { command: None };

    assert_eq!(config_operation(args).unwrap(), ConfigOperation::Show);
}

#[test]
fn provider_add_operation_parses_named_fields() {
    let args = ConfigArgs {
        command: Some(ConfigCommand::Provider {
            command: ConfigProviderCommand::Add {
                fields: vec![
                    "name:foo".to_owned(),
                    "type:openrouter".to_owned(),
                    "apikey:secret".to_owned(),
                ],
            },
        }),
    };

    assert_eq!(
        config_operation(args).unwrap(),
        ConfigOperation::AddProvider {
            name: "foo".to_owned(),
            provider_type: "openrouter".to_owned(),
            apikey: "secret".to_owned()
        }
    );
}

#[test]
fn model_add_operation_parses_expanded_reasoning() {
    let args = ConfigArgs {
        command: Some(ConfigCommand::Model {
            command: ConfigModelCommand::Add {
                fields: vec![
                    "provider:foo".to_owned(),
                    "id:openai/gpt-5.5".to_owned(),
                    "reasoning:xhigh".to_owned(),
                    "name:main".to_owned(),
                ],
            },
        }),
    };

    assert_eq!(
        config_operation(args).unwrap(),
        ConfigOperation::AddModel {
            provider: "foo".to_owned(),
            model_id: "openai/gpt-5.5".to_owned(),
            reasoning: ReasoningLevel::Xhigh,
            name: Some("main".to_owned())
        }
    );
}

#[test]
fn config_show_masks_api_keys() {
    let output = handle_config_with_io(
        ConfigArgs { command: None },
        || Ok(complete_config()),
        || Ok(ModelCache::default()),
        || Ok(None),
        |_| panic!("show must not write config"),
    )
    .unwrap();
    let output = strip_ansi_codes(&output);

    assert!(output.contains("Spectacular config"));
    assert!(output.contains("foo type: openrouter sk-o...test"));
    assert!(!output.contains("sk-or-v1-test"));
    assert!(output.contains("general: coding-bot"));
    assert!(output.contains("coding: coding-bot"));
    assert!(output.contains("labeling: label-bot"));
}

#[test]
fn provider_add_saves_config_without_leaking_key() {
    let saved_config = RefCell::new(None);

    let output = handle_config_with_io(
        ConfigArgs {
            command: Some(ConfigCommand::Provider {
                command: ConfigProviderCommand::Add {
                    fields: vec![
                        "name:foo".to_owned(),
                        "type:openrouter".to_owned(),
                        "apikey:sk-or-v1-test".to_owned(),
                    ],
                },
            }),
        },
        || Ok(SpectacularConfig::default()),
        || Ok(ModelCache::default()),
        || Ok(None),
        |config| {
            saved_config.replace(Some(config.clone()));
            Ok(())
        },
    )
    .unwrap();
    let output = strip_ansi_codes(&output);

    assert!(output.contains("Provider added"));
    assert!(!output.contains("sk-or-v1-test"));
    assert_eq!(
        saved_config
            .into_inner()
            .unwrap()
            .providers
            .get("foo")
            .unwrap()
            .apikey,
        "sk-or-v1-test"
    );
}

#[test]
fn provider_remove_requires_confirmation_without_write() {
    let wrote = RefCell::new(false);
    let output = handle_config_with_io(
        ConfigArgs {
            command: Some(ConfigCommand::Provider {
                command: ConfigProviderCommand::Remove {
                    fields: vec!["name:foo".to_owned()],
                },
            }),
        },
        || Ok(complete_config()),
        || Ok(ModelCache::default()),
        || Ok(None),
        |_| {
            wrote.replace(true);
            Ok(())
        },
    )
    .unwrap();
    let output = strip_ansi_codes(&output);

    assert!(output.contains("confirmation required"));
    assert!(!wrote.into_inner());
}

#[test]
fn model_add_requires_cached_api_metadata() {
    let error = handle_config_with_io(
        ConfigArgs {
            command: Some(ConfigCommand::Model {
                command: ConfigModelCommand::Add {
                    fields: vec![
                        "provider:foo".to_owned(),
                        "id:openai/gpt-5.5".to_owned(),
                        "reasoning:high".to_owned(),
                    ],
                },
            }),
        },
        || Ok(provider_only_config()),
        || Ok(ModelCache::default()),
        || Ok(None),
        |_| panic!("invalid model must not be written"),
    )
    .unwrap_err();

    assert!(
        matches!(error, AppError::InvalidConfigCommand(message) if message.contains("metadata cache"))
    );
}

#[test]
fn model_add_rejects_reasoning_without_supported_parameter() {
    let error = handle_config_with_io(
        ConfigArgs {
            command: Some(ConfigCommand::Model {
                command: ConfigModelCommand::Add {
                    fields: vec![
                        "provider:foo".to_owned(),
                        "id:openai/gpt-5.5".to_owned(),
                        "reasoning:high".to_owned(),
                    ],
                },
            }),
        },
        || Ok(provider_only_config()),
        || Ok(cache_with_model(false)),
        || Ok(None),
        |_| panic!("invalid reasoning must not be written"),
    )
    .unwrap_err();

    assert!(
        matches!(error, AppError::InvalidConfigCommand(message) if message.contains("reasoning"))
    );
}

#[test]
fn model_add_saves_when_cache_allows_reasoning() {
    let saved_config = RefCell::new(None);
    let output = handle_config_with_io(
        ConfigArgs {
            command: Some(ConfigCommand::Model {
                command: ConfigModelCommand::Add {
                    fields: vec![
                        "provider:foo".to_owned(),
                        "id:openai/gpt-5.5".to_owned(),
                        "reasoning:high".to_owned(),
                        "name:main".to_owned(),
                    ],
                },
            }),
        },
        || Ok(provider_only_config()),
        || Ok(cache_with_model(true)),
        || Ok(None),
        |config| {
            saved_config.replace(Some(config.clone()));
            Ok(())
        },
    )
    .unwrap();

    assert!(strip_ansi_codes(&output).contains("Model added"));
    assert!(saved_config
        .into_inner()
        .unwrap()
        .models
        .contains_key("main"));
}

#[test]
fn task_set_saves_model_reference() {
    let saved_config = RefCell::new(None);
    let output = handle_config_with_io(
        ConfigArgs {
            command: Some(ConfigCommand::Task {
                command: ConfigTaskCommand::Set {
                    fields: vec!["task:general".to_owned(), "model:coding-bot".to_owned()],
                },
            }),
        },
        || Ok(complete_config()),
        || Ok(ModelCache::default()),
        || Ok(None),
        |config| {
            saved_config.replace(Some(config.clone()));
            Ok(())
        },
    )
    .unwrap();

    assert!(strip_ansi_codes(&output).contains("Task model assigned"));
    assert_eq!(
        saved_config.into_inner().unwrap().tasks.general.as_deref(),
        Some("coding-bot")
    );
}

#[test]
fn plan_reports_missing_config_before_placeholder_output() {
    let output = handle_plan_with_loader("Create a login flow", || {
        Err(ConfigError::MissingConfigFile {
            path: "config.json".into(),
        })
    })
    .unwrap_err();

    assert!(matches!(
        output,
        AppError::Plan(PlanError::Config(ConfigError::MissingConfigFile { .. }))
    ));
}

#[test]
fn plan_with_complete_config_prints_placeholder_output() {
    let output = handle_plan_with_loader("Create a login flow", || Ok(complete_config())).unwrap();

    assert_eq!(output, "Hello World");
}

#[test]
fn incomplete_config_errors_tell_user_to_run_new_config_commands() {
    let error = AppError::Plan(PlanError::Config(ConfigError::MissingTaskModel {
        slot: TaskModelSlot::Coding,
    }));

    assert!(user_facing_error(&error).contains("spectacular config task set"));
}

fn provider_only_config() -> SpectacularConfig {
    let mut config = SpectacularConfig::default();
    config
        .add_provider("foo", "openrouter", "sk-or-v1-test")
        .unwrap();
    config
}

fn complete_config() -> SpectacularConfig {
    let mut config = provider_only_config();
    config.models.insert(
        "coding-bot".to_owned(),
        ModelConfig::new("foo", "openai/gpt-5.5", ReasoningLevel::High),
    );
    config.models.insert(
        "label-bot".to_owned(),
        ModelConfig::new("foo", "openai/gpt-5.5-mini", ReasoningLevel::None),
    );
    config.tasks = TaskAssignments {
        general: Some("coding-bot".to_owned()),
        coding: Some("coding-bot".to_owned()),
        labeling: Some("label-bot".to_owned()),
    };
    config
}

fn cache_with_model(supports_reasoning: bool) -> ModelCache {
    let supported_parameters = if supports_reasoning {
        vec!["reasoning".to_owned()]
    } else {
        Vec::new()
    };
    let mut providers = BTreeMap::new();
    let mut models = BTreeMap::new();
    models.insert(
        "openai/gpt-5.5".to_owned(),
        CachedModelMetadata::new("openai/gpt-5.5", "GPT-5.5", supported_parameters),
    );
    providers.insert(
        "foo".to_owned(),
        ProviderModelCache {
            provider_type: "openrouter".to_owned(),
            fetched_at: 42,
            models,
        },
    );
    ModelCache { providers }
}

fn strip_ansi_codes(value: &str) -> String {
    let mut output = String::new();
    let mut chars = value.chars().peekable();

    while let Some(character) = chars.next() {
        if character != '\u{1b}' {
            output.push(character);
            continue;
        }

        if chars.peek() == Some(&'[') {
            chars.next();
            for code_character in chars.by_ref() {
                if code_character.is_ascii_alphabetic() {
                    break;
                }
            }
        }
    }

    output
}
