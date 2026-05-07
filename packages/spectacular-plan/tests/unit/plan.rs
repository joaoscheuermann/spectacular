use super::*;
use spectacular_config::{
    ModelConfig, ProviderConfig, ReasoningLevel, TaskAssignments, TaskModelSlot,
};
use std::collections::BTreeMap;

#[test]
fn rejects_whitespace_only_prompt_before_loading_config() {
    let output = run("   ", || panic!("config should not be loaded"));

    assert!(matches!(output, Err(PlanError::EmptyPrompt)));
}

#[test]
fn returns_config_errors_for_missing_config() {
    let output = run("Create a login flow", || {
        Err(ConfigError::MissingConfigFile {
            path: "config.json".into(),
        })
    });

    assert!(matches!(
        output,
        Err(PlanError::Config(ConfigError::MissingConfigFile { .. }))
    ));
}

#[test]
fn rejects_incomplete_config() {
    let output = run("Create a login flow", || {
        let mut config = complete_config();
        config.tasks.coding = None;
        Ok(config)
    });

    assert!(matches!(
        output,
        Err(PlanError::Config(ConfigError::MissingTaskModel {
            slot: TaskModelSlot::Coding
        }))
    ));
}

#[test]
fn complete_config_prints_placeholder_output() {
    let output = run("Create a login flow", || Ok(complete_config())).unwrap();

    assert_eq!(output, "Hello World");
}

fn complete_config() -> SpectacularConfig {
    let mut providers = BTreeMap::new();
    providers.insert(
        "foo".to_owned(),
        ProviderConfig::new("openrouter", "sk-or-v1-test"),
    );
    let mut models = BTreeMap::new();
    models.insert(
        "general-model".to_owned(),
        ModelConfig::new("foo", "openrouter/general", ReasoningLevel::None),
    );
    models.insert(
        "coding-model".to_owned(),
        ModelConfig::new("foo", "openrouter/coding", ReasoningLevel::None),
    );
    models.insert(
        "labeling-model".to_owned(),
        ModelConfig::new("foo", "openrouter/labeling", ReasoningLevel::None),
    );

    SpectacularConfig {
        providers,
        models,
        tasks: TaskAssignments {
            general: Some("general-model".to_owned()),
            coding: Some("coding-model".to_owned()),
            labeling: Some("labeling-model".to_owned()),
        },
    }
}
