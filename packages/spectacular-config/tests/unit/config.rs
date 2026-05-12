use super::*;

/// Verifies that default config is incomplete without tasks.
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

/// Verifies that config round trips flat schema.
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

/// Verifies that config round trips chatgpt provider auth.
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

/// Verifies that complete config accepts provider with chatgpt auth without API key.
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

/// Verifies that legacy top level schema fails clearly.
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

/// Verifies that legacy nested schema fails clearly.
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

/// Verifies that complete config requires task references to saved models.
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

/// Verifies that complete config requires model provider to exist.
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

/// Verifies that complete config passes validation.
#[test]
fn complete_config_passes_validation() {
    let config = complete_config();

    assert!(config.validate_complete().is_ok());
    assert!(config.is_complete());
}

/// Verifies that add model uses composite key by default.
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

/// Verifies that add model allows custom key and keeps internal key.
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

/// Verifies that model removal preserves task references.
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

/// Verifies that backup config file copies existing config.
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

/// Verifies that model cache round trips.
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
        )
        .with_context_window_tokens(Some(128_000))],
    );

    write_model_cache_to_path(&path, &cache).unwrap();
    let loaded = read_model_cache_from_path(&path).unwrap();

    assert_eq!(loaded, cache);
    assert!(loaded
        .model("foo", "openai/gpt-5.5")
        .unwrap()
        .supports_reasoning());
    assert_eq!(
        loaded
            .model("foo", "openai/gpt-5.5")
            .unwrap()
            .context_window_tokens,
        Some(128_000)
    );

    let _ = fs::remove_file(path);
}

/// Verifies that model cache accepts entries without context window.
#[test]
fn model_cache_accepts_entries_without_context_window() {
    let path = temp_config_path("cache-old").with_file_name(MODEL_CACHE_FILE_NAME);
    write_text(
        &path,
        r#"{
  "providers": {
    "foo": {
      "provider_type": "openrouter",
      "fetched_at": 42,
      "models": {
        "openai/gpt-5.5": {
          "id": "openai/gpt-5.5",
          "name": "GPT-5.5",
          "supported_parameters": ["reasoning", "tools"]
        }
      }
    }
  }
}"#,
    );

    let loaded = read_model_cache_from_path(&path).unwrap();

    assert_eq!(
        loaded
            .model("foo", "openai/gpt-5.5")
            .unwrap()
            .context_window_tokens,
        None
    );

    let _ = fs::remove_file(path);
}

/// Verifies that parsing accepts expanded reasoning values.
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

/// Verifies that parsing rejects old planning task name.
#[test]
fn parsing_rejects_old_planning_task_name() {
    let error = "planning".parse::<TaskModelSlot>().unwrap_err();

    assert!(matches!(error, ConfigParseError::InvalidTask { .. }));
}

/// Verifies that API key masking never returns full key.
#[test]
fn api_key_masking_never_returns_full_key() {
    assert_eq!(mask_api_key("sk-or-v1-test"), "sk-o...test");
    assert_ne!(mask_api_key("sk-or-v1-test"), "sk-or-v1-test");
}

/// Verifies that windows config dir uses appdata.
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

/// Builds a complete configuration for test scenarios.
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

/// Builds a temporary configuration path for a named test case.
fn temp_config_path(name: &str) -> PathBuf {
    let suffix = timestamp();

    env::temp_dir()
        .join(format!("spectacular-config-test-{name}-{suffix}"))
        .join(CONFIG_FILE_NAME)
}

/// Writes test fixture text to the supplied path.
fn write_text(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }

    fs::write(path, content).unwrap();
}
