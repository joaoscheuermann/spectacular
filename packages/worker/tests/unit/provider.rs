use std::path::Path;
use std::sync::{Arc, Mutex};

use agent::AgentConfig;
use config::{
    CachedModelMetadata, ChatGptAuthConfig, DoricConfig, ModelCache, ProviderConfig,
    ProviderCredentials, ReasoningLevel, TaskModelSlot,
};
use llms::{
    LlmProvider, OpenAiAuthRecord, ProviderError, OPENAI_PROVIDER_ID, OPENROUTER_PROVIDER_ID,
};
use worker::error::{WorkerError, WorkerFailureReason, WorkerResult};
use worker::provider::{
    agent_config_for_runtime, provider_for_runtime, select_runtime, WorkerConfigIo,
    WorkerOpenAiAuthStore, WorkerProviderAuth, WorkerProviderDeps, WorkerRuntimeSelection,
};

#[test]
fn select_runtime_configured_coding_model_returns_provider_model_reasoning_auth_and_cache() {
    let (config, cache) = configured_openrouter(ReasoningLevel::High, Some(200_000));

    let runtime = select_runtime(&config, &cache).unwrap();

    assert_eq!(runtime.provider_name, "openrouter-main");
    assert_eq!(runtime.provider_type, OPENROUTER_PROVIDER_ID);
    assert_eq!(runtime.model_key, "coding-fast");
    assert_eq!(runtime.model, "openai/gpt-5.1-codex");
    assert_eq!(runtime.reasoning, ReasoningLevel::High);
    assert_eq!(runtime.context_window_tokens, Some(200_000));
    assert_eq!(
        runtime.provider_auth,
        WorkerProviderAuth::ApiKey {
            api_key: "sk-test_openrouter".to_owned()
        }
    );
}

#[test]
fn select_runtime_missing_cache_entry_succeeds_without_context_window() {
    let (config, cache) = configured_openrouter(ReasoningLevel::Medium, None);

    let runtime = select_runtime(&config, &cache).unwrap();

    assert_eq!(runtime.context_window_tokens, None);
}

#[test]
fn agent_config_for_runtime_applies_model_reasoning_and_context_window() {
    let runtime = runtime_selection(
        OPENROUTER_PROVIDER_ID,
        "openrouter-main",
        "openai/gpt-5.1-codex",
        ReasoningLevel::High,
        WorkerProviderAuth::ApiKey {
            api_key: "sk-test_openrouter".to_owned(),
        },
    )
    .with_context_window_tokens(Some(200_000));

    let config = agent_config_for_runtime(&runtime, "system prompt");

    assert_eq!(config.system_prompt, "system prompt");
    assert_eq!(config.model.as_deref(), Some("openai/gpt-5.1-codex"));
    assert!(config.include_reasoning);
    assert_eq!(config.reasoning_effort.as_deref(), Some("high"));
    assert_eq!(
        config.context_policy.model_context_window_tokens,
        Some(200_000)
    );
}

#[test]
fn agent_config_for_runtime_none_reasoning_disables_reasoning_request() {
    let runtime = runtime_selection(
        OPENAI_PROVIDER_ID,
        "openai",
        "gpt-5.1-codex",
        ReasoningLevel::None,
        WorkerProviderAuth::ApiKey {
            api_key: "sk-test_openai".to_owned(),
        },
    );

    let config = agent_config_for_runtime(&runtime, "");

    assert_eq!(config.model.as_deref(), Some("gpt-5.1-codex"));
    assert!(!config.include_reasoning);
    assert_eq!(config.reasoning_effort, None);
}

#[test]
fn provider_for_runtime_openrouter_api_key_constructs_openrouter_provider_without_network() {
    let runtime = runtime_selection(
        OPENROUTER_PROVIDER_ID,
        "openrouter-main",
        "openai/gpt-5.1-codex",
        ReasoningLevel::Medium,
        WorkerProviderAuth::ApiKey {
            api_key: "sk-test_openrouter".to_owned(),
        },
    );

    let provider = provider_for_runtime(&runtime, WorkerProviderDeps::offline()).unwrap();

    assert_eq!(provider.metadata().id(), OPENROUTER_PROVIDER_ID);
    assert_eq!(provider.metadata().display_name(), "OpenRouter");
}

#[test]
fn provider_for_runtime_openai_api_key_constructs_openai_provider_without_network() {
    let runtime = runtime_selection(
        OPENAI_PROVIDER_ID,
        "openai",
        "gpt-5.1-codex",
        ReasoningLevel::High,
        WorkerProviderAuth::ApiKey {
            api_key: "sk-test_openai".to_owned(),
        },
    );

    let provider = provider_for_runtime(&runtime, WorkerProviderDeps::offline()).unwrap();

    assert_eq!(provider.metadata().id(), OPENAI_PROVIDER_ID);
    assert_eq!(provider.metadata().display_name(), "OpenAI");
}

#[test]
fn worker_openai_auth_store_loads_and_saves_oauth_for_named_provider() {
    let io = Arc::new(InMemoryConfigIo::new(config_with_openai_oauth(
        auth_config("id.initial", "access.initial", "refresh.initial"),
    )));
    let store = WorkerOpenAiAuthStore::new("openai", io.clone());

    let loaded = store.load_openai_auth().unwrap();
    assert_auth_record(&loaded, "id.initial", "access.initial", "refresh.initial");

    store
        .save_openai_auth(auth_record("id.next", "access.next", "refresh.next"))
        .unwrap();

    let saved = io.written_config().unwrap();
    let provider = saved.providers.get("openai").unwrap();
    assert_eq!(
        provider.oauth_config().unwrap(),
        auth_config("id.next", "access.next", "refresh.next")
    );
}

#[test]
fn worker_openai_auth_store_missing_oauth_returns_authentication_required_without_tokens() {
    let io = Arc::new(InMemoryConfigIo::new(config_with_openai_api_key()));
    let store = WorkerOpenAiAuthStore::new("openai", io);

    let error = store.load_openai_auth().unwrap_err();

    assert_eq!(
        error,
        ProviderError::AuthenticationRequired {
            provider_name: "OpenAI".to_owned()
        }
    );
    assert_redacted(&error.to_string());
}

#[test]
fn provider_errors_map_to_redacted_lifecycle_ready_worker_failures() {
    let cases = [
        (
            provider_for_runtime(
                &runtime_selection(
                    "anthropic",
                    "claude",
                    "claude-3.7",
                    ReasoningLevel::Medium,
                    WorkerProviderAuth::ApiKey {
                        api_key: "sk-test_unsupported".to_owned(),
                    },
                ),
                WorkerProviderDeps::offline(),
            )
            .unwrap_err(),
            WorkerFailureReason::ProviderUnsupported,
        ),
        (
            select_runtime(&config_missing_credentials(), &ModelCache::default()).unwrap_err(),
            WorkerFailureReason::ProviderCredentials,
        ),
        (
            provider_for_runtime(
                &runtime_selection(
                    OPENROUTER_PROVIDER_ID,
                    "openrouter-main",
                    "openai/gpt-5.1-codex",
                    ReasoningLevel::Medium,
                    WorkerProviderAuth::Oauth {
                        provider_name: "openrouter-main".to_owned(),
                    },
                ),
                WorkerProviderDeps::offline(),
            )
            .unwrap_err(),
            WorkerFailureReason::ProviderUnsupportedAuth,
        ),
        (
            WorkerError::provider_setup_failed(
                "sk-test_secret_1234567890abcdef access_token refresh_token",
            ),
            WorkerFailureReason::ProviderSetupFailed,
        ),
    ];

    for (error, expected) in cases {
        assert_eq!(error.lifecycle_failure_reason(), expected);
        assert_redacted(&error.lifecycle_failure_message());
    }
}

#[test]
fn select_runtime_missing_coding_model_returns_provider_configuration_failure() {
    let mut config = config_with_openrouter(ReasoningLevel::High);
    config.tasks.coding = None;

    let error = select_runtime(&config, &ModelCache::default()).unwrap_err();

    assert_eq!(
        error.lifecycle_failure_reason(),
        WorkerFailureReason::ProviderConfiguration
    );
    assert_redacted(&error.lifecycle_failure_message());
}

#[test]
fn provider_agent_config_contract_stays_compatible_with_agent_config_defaults() {
    let runtime = runtime_selection(
        OPENAI_PROVIDER_ID,
        "openai",
        "gpt-5.1-codex",
        ReasoningLevel::None,
        WorkerProviderAuth::ApiKey {
            api_key: "sk-test_openai".to_owned(),
        },
    );

    let actual = agent_config_for_runtime(&runtime, "");
    let expected = AgentConfig {
        model: Some("gpt-5.1-codex".to_owned()),
        context_policy: Default::default(),
        ..AgentConfig::default()
    };

    assert_eq!(actual, expected);
}

#[test]
fn provider_manifest_does_not_depend_on_cli() {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = std::fs::read_to_string(manifest_path).unwrap();

    assert!(!manifest.lines().any(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with("cli ") || trimmed.starts_with("cli=")
    }));
}

fn configured_openrouter(
    reasoning: ReasoningLevel,
    context_window_tokens: Option<usize>,
) -> (DoricConfig, ModelCache) {
    let config = config_with_openrouter(reasoning);
    let mut cache = ModelCache::default();

    if context_window_tokens.is_some() {
        cache.put_provider(
            "openrouter-main",
            OPENROUTER_PROVIDER_ID,
            1_700_000_000,
            [CachedModelMetadata::new(
                "openai/gpt-5.1-codex",
                "GPT 5.1 Codex",
                ["reasoning".to_owned()],
            )
            .with_context_window_tokens(context_window_tokens)],
        );
    }

    (config, cache)
}

fn config_with_openrouter(reasoning: ReasoningLevel) -> DoricConfig {
    let mut config = DoricConfig::default();
    config.providers.insert(
        "openrouter-main".to_owned(),
        ProviderConfig::new(OPENROUTER_PROVIDER_ID, "sk-test_openrouter"),
    );
    let model_key = config
        .add_model(
            "openrouter-main",
            "openai/gpt-5.1-codex",
            reasoning,
            Some("coding-fast".to_owned()),
        )
        .unwrap();
    config
        .set_task_model(TaskModelSlot::Coding, model_key)
        .unwrap();
    config
}

fn config_missing_credentials() -> DoricConfig {
    let mut config = config_with_openrouter(ReasoningLevel::High);
    config.providers.insert(
        "openrouter-main".to_owned(),
        ProviderConfig {
            provider_type: OPENROUTER_PROVIDER_ID.to_owned(),
            credentials: None,
        },
    );
    config
}

fn config_with_openai_api_key() -> DoricConfig {
    let mut config = DoricConfig::default();
    config.providers.insert(
        "openai".to_owned(),
        ProviderConfig::new(OPENAI_PROVIDER_ID, "sk-test_openai"),
    );
    config
}

fn config_with_openai_oauth(auth: ChatGptAuthConfig) -> DoricConfig {
    let mut config = DoricConfig::default();
    config.providers.insert(
        "openai".to_owned(),
        ProviderConfig {
            provider_type: OPENAI_PROVIDER_ID.to_owned(),
            credentials: Some(ProviderCredentials::ChatGptOauth(auth)),
        },
    );
    config
}

fn runtime_selection(
    provider_type: &str,
    provider_name: &str,
    model: &str,
    reasoning: ReasoningLevel,
    provider_auth: WorkerProviderAuth,
) -> WorkerRuntimeSelection {
    WorkerRuntimeSelection {
        provider_type: provider_type.to_owned(),
        provider_name: provider_name.to_owned(),
        model_key: format!("{provider_name}/{model}"),
        model: model.to_owned(),
        reasoning,
        provider_auth,
        context_window_tokens: None,
    }
}

fn auth_config(id_token: &str, access_token: &str, refresh_token: &str) -> ChatGptAuthConfig {
    ChatGptAuthConfig {
        id_token: id_token.to_owned(),
        access_token: access_token.to_owned(),
        refresh_token: refresh_token.to_owned(),
        account_id: Some("account-id".to_owned()),
        email: Some("user@example.com".to_owned()),
        plan_type: Some("plus".to_owned()),
        user_id: Some("user-id".to_owned()),
        fedramp: false,
        last_refresh_epoch_seconds: 42,
    }
}

fn auth_record(id_token: &str, access_token: &str, refresh_token: &str) -> OpenAiAuthRecord {
    OpenAiAuthRecord {
        id_token: id_token.to_owned(),
        access_token: access_token.to_owned(),
        refresh_token: refresh_token.to_owned(),
        account_id: Some("account-id".to_owned()),
        email: Some("user@example.com".to_owned()),
        plan_type: Some("plus".to_owned()),
        user_id: Some("user-id".to_owned()),
        fedramp: false,
        last_refresh_epoch_seconds: 42,
    }
}

fn assert_auth_record(
    actual: &OpenAiAuthRecord,
    id_token: &str,
    access_token: &str,
    refresh_token: &str,
) {
    assert_eq!(actual.id_token, id_token);
    assert_eq!(actual.access_token, access_token);
    assert_eq!(actual.refresh_token, refresh_token);
    assert_eq!(actual.account_id.as_deref(), Some("account-id"));
    assert_eq!(actual.email.as_deref(), Some("user@example.com"));
    assert_eq!(actual.plan_type.as_deref(), Some("plus"));
    assert_eq!(actual.user_id.as_deref(), Some("user-id"));
    assert_eq!(actual.last_refresh_epoch_seconds, 42);
}

fn assert_redacted(message: &str) {
    for secret in [
        "sk-test",
        "access_token",
        "refresh_token",
        "access.initial",
        "refresh.initial",
        "access.next",
        "refresh.next",
    ] {
        assert!(
            !message.contains(secret),
            "`{secret}` leaked in `{message}`"
        );
    }
}

trait RuntimeSelectionExt {
    fn with_context_window_tokens(self, context_window_tokens: Option<usize>) -> Self;
}

impl RuntimeSelectionExt for WorkerRuntimeSelection {
    fn with_context_window_tokens(mut self, context_window_tokens: Option<usize>) -> Self {
        self.context_window_tokens = context_window_tokens;
        self
    }
}

struct InMemoryConfigIo {
    config: Mutex<DoricConfig>,
    written: Mutex<Option<DoricConfig>>,
}

impl InMemoryConfigIo {
    fn new(config: DoricConfig) -> Self {
        Self {
            config: Mutex::new(config),
            written: Mutex::new(None),
        }
    }

    fn written_config(&self) -> Option<DoricConfig> {
        self.written.lock().unwrap().clone()
    }
}

impl WorkerConfigIo for InMemoryConfigIo {
    fn read_config_or_default(&self) -> WorkerResult<DoricConfig> {
        Ok(self.config.lock().unwrap().clone())
    }

    fn write_config(&self, config: &DoricConfig) -> WorkerResult<()> {
        *self.written.lock().unwrap() = Some(config.clone());
        *self.config.lock().unwrap() = config.clone();
        Ok(())
    }
}
