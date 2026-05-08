use super::*;

#[test]
fn default_config_is_incomplete_without_tasks() {
    let error = SpectacularConfig::default()
        .validate_complete()
        .unwrap_err();

    assert!(matches!(
        error,
        ConfigError::MissingTaskModel {
            slot: TaskModelSlot::General
        }
    ));
}

#[test]
fn config_round_trips_flat_schema() {
    let path = temp_config_path("round-trip");
    let config = complete_config();

    write_config_to_path(&path, &config).unwrap();
    let loaded = read_config_from_path(&path).unwrap();

    assert_eq!(loaded, config);

    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains("\"providers\""));
    assert!(content.contains("\"models\""));
    assert!(content.contains("\"tasks\""));
    assert!(content.contains("\"type\""));
    assert!(content.contains("\"auth\": \"apikey\""));
    assert!(content.contains("\"apikey\""));
    assert!(!content.contains("\"selected\""));
    assert!(!content.contains("\"available\""));

    let _ = fs::remove_file(path);
}

#[test]
fn config_round_trips_chatgpt_provider_auth() {
    let path = temp_config_path("chatgpt-auth");
    let mut config = SpectacularConfig::default();
    config
        .set_provider_oauth(
            "openai",
            ChatGptAuthConfig {
                id_token: "id-token".to_owned(),
                access_token: "access-token".to_owned(),
                refresh_token: "refresh-token".to_owned(),
                account_id: Some("account-1".to_owned()),
                email: Some("user@example.com".to_owned()),
                plan_type: Some("plus".to_owned()),
                user_id: Some("user-1".to_owned()),
                fedramp: true,
                last_refresh_epoch_seconds: 42,
            },
        )
        .unwrap();

    write_config_to_path(&path, &config).unwrap();
    let loaded = read_config_from_path(&path).unwrap();

    assert_eq!(loaded, config);
    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains("\"auth\": \"oauth\""));
    assert!(content.contains("\"access_token\": \"access-token\""));
    assert!(!content.contains("\"apikey\""));

    let _ = fs::remove_file(path);
}

#[test]
fn complete_config_accepts_provider_with_chatgpt_auth_without_api_key() {
    let mut config = SpectacularConfig::default();
    config
        .set_provider_oauth(
            "openai",
            ChatGptAuthConfig {
                id_token: "id-token".to_owned(),
                access_token: "access-token".to_owned(),
                refresh_token: "refresh-token".to_owned(),
                account_id: None,
                email: None,
                plan_type: None,
                user_id: None,
                fedramp: false,
                last_refresh_epoch_seconds: 42,
            },
        )
        .unwrap();
    config
        .add_model(
            "openai",
            "gpt-5.5",
            ReasoningLevel::High,
            Some("coding-bot".to_owned()),
        )
        .unwrap();
    config
        .set_task_model(TaskModelSlot::General, "coding-bot")
        .unwrap();
    config
        .set_task_model(TaskModelSlot::Coding, "coding-bot")
        .unwrap();
    config
        .set_task_model(TaskModelSlot::Labeling, "coding-bot")
        .unwrap();

    assert!(config.validate_complete().is_ok());
}

#[test]
fn legacy_top_level_schema_fails_clearly() {
    let path = temp_config_path("legacy-top");
    write_text(
        &path,
        r#"{
  "selected_provider": "openrouter",
  "provider_api_keys": {"openrouter": "sk-or-v1-test"}
}"#,
    );

    let error = read_config_from_path(&path).unwrap_err();

    assert_eq!(error.to_string(), SCHEMA_CHANGED_MESSAGE);

    let _ = fs::remove_file(path);
}

#[test]
fn legacy_nested_schema_fails_clearly() {
    let path = temp_config_path("legacy-nested");
    write_text(
        &path,
        r#"{
  "providers": {
    "selected": "openrouter",
    "available": {}
  }
}"#,
    );

    let error = read_config_from_path(&path).unwrap_err();

    assert_eq!(error.to_string(), SCHEMA_CHANGED_MESSAGE);

    let _ = fs::remove_file(path);
}

#[test]
fn complete_config_requires_task_references_to_saved_models() {
    let mut config = complete_config();
    config.tasks.coding = Some("missing".to_owned());

    let error = config.validate_complete().unwrap_err();

    assert!(matches!(
        error,
        ConfigError::InvalidTaskModelReference {
            slot: TaskModelSlot::Coding,
            model
        } if model == "missing"
    ));
}

#[test]
fn complete_config_requires_model_provider_to_exist() {
    let mut config = complete_config();
    config.providers.remove("foo");

    let error = config.validate_complete().unwrap_err();

    assert!(matches!(
        error,
        ConfigError::ModelProviderNotConfigured { provider, .. } if provider == "foo"
    ));
}

#[test]
fn complete_config_passes_validation() {
    let config = complete_config();

    assert!(config.validate_complete().is_ok());
    assert!(config.is_complete());
}

#[test]
fn add_model_uses_composite_key_by_default() {
    let mut config = SpectacularConfig::default();
    config
        .add_provider("foo", "openrouter", "sk-or-v1-test")
        .unwrap();

    let key = config
        .add_model("foo", "openai/gpt-5.5", ReasoningLevel::High, None)
        .unwrap();

    assert_eq!(key, "foo:openai/gpt-5.5");
    assert_eq!(
        config.models[&key].internal_key,
        "foo:openai/gpt-5.5".to_owned()
    );
}

#[test]
fn add_model_allows_custom_key_and_keeps_internal_key() {
    let mut config = SpectacularConfig::default();
    config
        .add_provider("foo", "openrouter", "sk-or-v1-test")
        .unwrap();

    let key = config
        .add_model(
            "foo",
            "openai/gpt-5.5",
            ReasoningLevel::Xhigh,
            Some("coding-bot".to_owned()),
        )
        .unwrap();

    assert_eq!(key, "coding-bot");
    assert_eq!(
        config.models["coding-bot"].internal_key,
        "foo:openai/gpt-5.5"
    );
}

#[test]
fn model_removal_preserves_task_references() {
    let mut config = complete_config();
    let referenced_by = config.tasks.references_to("coding-bot");

    assert_eq!(
        referenced_by,
        vec![TaskModelSlot::General, TaskModelSlot::Coding]
    );

    config.remove_model("coding-bot").unwrap();

    assert_eq!(config.tasks.general.as_deref(), Some("coding-bot"));
    assert_eq!(config.tasks.coding.as_deref(), Some("coding-bot"));
}

#[test]
fn backup_config_file_copies_existing_config() {
    let path = temp_config_path("backup");
    write_config_to_path(&path, &complete_config()).unwrap();

    let backup = backup_config_file(&path).unwrap().unwrap();

    assert!(backup.exists());
    assert!(backup
        .file_name()
        .unwrap()
        .to_string_lossy()
        .contains(BACKUP_EXTENSION));

    let _ = fs::remove_file(path);
    let _ = fs::remove_file(backup);
}

#[test]
fn model_cache_round_trips() {
    let path = temp_config_path("cache").with_file_name(MODEL_CACHE_FILE_NAME);
    let mut cache = ModelCache::default();
    cache.put_provider(
        "foo",
        "openrouter",
        42,
        [CachedModelMetadata::new(
            "openai/gpt-5.5",
            "GPT-5.5",
            ["reasoning".to_owned(), "tools".to_owned()],
        )],
    );

    write_model_cache_to_path(&path, &cache).unwrap();
    let loaded = read_model_cache_from_path(&path).unwrap();

    assert_eq!(loaded, cache);
    assert!(loaded
        .model("foo", "openai/gpt-5.5")
        .unwrap()
        .supports_reasoning());

    let _ = fs::remove_file(path);
}

#[test]
fn parsing_accepts_expanded_reasoning_values() {
    assert_eq!(
        "minimal".parse::<ReasoningLevel>().unwrap(),
        ReasoningLevel::Minimal
    );
    assert_eq!(
        "xhigh".parse::<ReasoningLevel>().unwrap(),
        ReasoningLevel::Xhigh
    );
}

#[test]
fn parsing_rejects_old_planning_task_name() {
    let error = "planning".parse::<TaskModelSlot>().unwrap_err();

    assert!(matches!(error, ConfigParseError::InvalidTask { .. }));
}

#[test]
fn api_key_masking_never_returns_full_key() {
    assert_eq!(mask_api_key("sk-or-v1-test"), "sk-o...test");
    assert_ne!(mask_api_key("sk-or-v1-test"), "sk-or-v1-test");
}

#[cfg(windows)]
#[test]
fn windows_config_dir_uses_appdata() {
    let appdata = PathBuf::from(r"C:\Users\example\AppData\Roaming");
    let config_dir = config_dir_with_env(|key| {
        if key == "APPDATA" {
            Some(appdata.clone().into_os_string())
        } else {
            None
        }
    })
    .unwrap();

    assert_eq!(config_dir, appdata.join(APP_CONFIG_DIR_NAME));
}

fn complete_config() -> SpectacularConfig {
    let mut config = SpectacularConfig::default();
    config
        .add_provider("foo", "openrouter", "sk-or-v1-test")
        .unwrap();
    config
        .add_model(
            "foo",
            "openai/gpt-5.5",
            ReasoningLevel::High,
            Some("coding-bot".to_owned()),
        )
        .unwrap();
    config
        .add_model("foo", "openai/gpt-5.5-mini", ReasoningLevel::None, None)
        .unwrap();
    config
        .set_task_model(TaskModelSlot::General, "coding-bot")
        .unwrap();
    config
        .set_task_model(TaskModelSlot::Coding, "coding-bot")
        .unwrap();
    config
        .set_task_model(TaskModelSlot::Labeling, "foo:openai/gpt-5.5-mini")
        .unwrap();
    config
}

fn temp_config_path(name: &str) -> PathBuf {
    let suffix = timestamp();

    env::temp_dir()
        .join(format!("spectacular-config-test-{name}-{suffix}"))
        .join(CONFIG_FILE_NAME)
}

fn write_text(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }

    fs::write(path, content).unwrap();
}
