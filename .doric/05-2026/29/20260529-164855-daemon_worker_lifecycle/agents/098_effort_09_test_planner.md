# Agent Receipt: effort 09 test planner

## Spawn proof
- Tool: multi_agent_v1.spawn_agent
- Agent type: worker
- Agent id: 019e76f0-60d7-7622-bfeb-77a1019fa213
- Spawn result: spawned
- Required agents row: `development | efforts/09_worker_provider_runtime.md | test planner | worker | agents/098_effort_09_test_planner.md`

## Role

Development test-planner sub-agent for effort 09. This role translates the accepted worker provider/runtime acceptance criteria into concrete tests, focused red/green commands, and expected API seams. This receipt is planning-only and intentionally does not edit production or test code.

## Input artifacts

- `.agents/skills/coding-conventions/SKILL.md`
- `.agents/skills/coding-conventions/references/implementation-standards.md`
- `.agents/skills/coding-conventions/references/sexy-rust.md`
- `.agents/skills/coding-conventions/references/monodon-rust.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/STATE.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/efforts/09_worker_provider_runtime.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/TDD.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/FEATURES.md`

## Read ownership

- Worker current files:
  - `packages/worker/Cargo.toml`
  - `packages/worker/project.json`
  - `packages/worker/src/lib.rs`
  - `packages/worker/src/error.rs`
  - `packages/worker/src/repo.rs`
  - `packages/worker/src/state.rs`
  - `packages/worker/tests/unit.rs`
  - `packages/worker/tests/unit/repo.rs`
- Provider/config/agent read-only context:
  - `packages/config/src/lib.rs`
  - `packages/config/src/schema.rs`
  - `packages/config/src/provider_schema.rs`
  - `packages/config/src/config_model.rs`
  - `packages/config/src/errors.rs`
  - `packages/llms/src/lib.rs`
  - `packages/llms/src/openai/auth.rs`
  - `packages/llms/src/openai/mod.rs`
  - `packages/llms/src/openrouter/mod.rs`
  - `packages/llms/src/debug_log.rs`
  - `packages/llms/src/provider.rs`
  - `packages/llms/src/registry.rs`
  - `packages/llms/src/types/error.rs`
  - `packages/llms/src/types/capabilities.rs`
  - `packages/llms/src/types/stream.rs`
  - `packages/llms/src/types/messages.rs`
  - `packages/agent/src/agent.rs`
  - `packages/agent/src/context.rs`
  - `packages/agent/src/context/policy.rs`
  - `packages/agent/src/store.rs`
  - `packages/cli/src/chat/runtime_selection.rs`
  - `packages/cli/src/chat/provider.rs`
  - `packages/cli/src/chat/auth.rs`

## Write ownership

- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/098_effort_09_test_planner.md` only.

## Coding conventions

- `SKILL.md`: Applied the repository invariants for simplicity, deep module boundaries, explicit dependency injection, behavior-oriented tests, and Red-Green-Refactor. For this effort, this means the test plan should drive a small worker-local boundary instead of speculative runtime extraction.
- `references/implementation-standards.md`: Applied package-root `tests/` placement, no inline production tests except minimal include harnesses, F.I.R.S.T., Arrange-Act-Assert, behavior/public-contract testing, explicit dependency injection, and file-size awareness. New worker tests should live under `packages/worker/tests/unit/provider.rs` and be included by `packages/worker/tests/unit.rs`.
- `references/sexy-rust.md`: Applied Rust-specific guidance for type-driven boundaries, flat `Result`/`Option` control flow, natural API conversions, and narrow side effects. The provider plan should prefer typed `WorkerRuntimeSelection`, provider enum/wrapper, and error variants over stringly scattered branches.
- `references/monodon-rust.md`: Applied Rust/Nx target expectations for package validation. The focused commands should use Cargo package gates first, with `worker:test`/`worker:lint` as Nx equivalents when available.

## Prompt summary

Plan tests for effort 09 worker-local provider/runtime composition. The worker must resolve the coding model from persisted config and model cache, build OpenRouter/OpenAI providers through public `llms` APIs or fakeable seams, persist OpenAI OAuth through a worker-local auth store if public APIs allow it, map config/provider failures into redacted lifecycle-ready worker errors, default debug logging to disabled, set agent reasoning/context policy, avoid a `cli` dependency, and avoid extracting a new runtime/provider-runtime package in v1.

## Output

### Test file and harness

- Add `packages/worker/tests/unit/provider.rs`.
- Update `packages/worker/tests/unit.rs` with `#[path = "unit/provider.rs"] mod provider;`.
- Keep test helpers local to `provider.rs` unless they become shared by a later effort.
- Test through public worker APIs, not private helpers. If the implementation needs helpers, expose a small worker-local public contract from `worker::provider`.

### Public API shape to drive from tests

If no provider API exists yet, the test writer should create this public worker-local shape:

- `worker::provider::WorkerRuntimeSelection`
  - fields or accessors for `provider_type`, `provider_name`, `model_key`, `model`, `reasoning`, `context_window_tokens`, and `provider_auth`.
  - built from `DoricConfig` and `ModelCache`, not from filesystem I/O.
- `worker::provider::select_runtime(config: &DoricConfig, cache: &ModelCache) -> WorkerResult<WorkerRuntimeSelection>`
  - uses `TaskModelSlot::Coding`, `DoricConfig::model_for_task`, `DoricConfig::provider_for_model`, `ProviderConfig::auth_mode`, `ProviderConfig::api_key`, `ProviderConfig::oauth_config`, `ModelCache::model`, and `ReasoningLevel`.
- `worker::provider::WorkerProvider`
  - enum or wrapper implementing `llms::LlmProvider` and delegating to `OpenRouterProvider` and `OpenAiProvider`.
- `worker::provider::provider_for_runtime(runtime: &WorkerRuntimeSelection, deps: WorkerProviderDeps) -> WorkerResult<WorkerProvider>`
  - `WorkerProviderDeps` should carry `LlmDebugLogger` and worker-local config read/write operations needed by OAuth.
  - Default production dependency should use `LlmDebugLogger::disabled()` unless a later worker runtime explicitly supplies a logger.
- `worker::provider::WorkerOpenAiAuthStore`
  - implements `llms::OpenAiAuthStore`.
  - loads `ChatGptAuthConfig` from the named provider and converts it to `OpenAiAuthRecord`.
  - saves refreshed `OpenAiAuthRecord` back through `DoricConfig::set_provider_oauth` and `config::write_config`.
- `worker::error::WorkerFailureReason` should gain provider/setup variants suitable for lifecycle events, for example `ProviderConfiguration`, `ProviderCredentials`, `ProviderUnsupported`, and `ProviderSetupFailed`, with redacted messages.

The implementation may use a trait for config I/O to keep tests repeatable:

```rust
pub trait WorkerConfigIo: Send + Sync {
    fn read_config_or_default(&self) -> Result<DoricConfig, config::ConfigError>;
    fn write_config(&self, config: &DoricConfig) -> Result<(), config::ConfigError>;
}
```

The production implementation can call `config::read_config_or_default` and `config::write_config`. Tests should use an in-memory fake.

### Required focused tests

- `select_runtime_configured_coding_model_returns_provider_model_reasoning_and_auth`
  - Arrange a `DoricConfig` with an OpenRouter provider, a saved coding model, `ReasoningLevel::High`, and a matching task assignment.
  - Assert the selected runtime uses `TaskModelSlot::Coding`, preserves model key/model id/provider name/provider type, sets API-key auth, and exposes `include_reasoning = true` or a value that maps to `AgentConfig.include_reasoning = true` and `reasoning_effort = Some("high")`.

- `select_runtime_cached_context_window_uses_model_cache_metadata`
  - Arrange a `ModelCache` entry for the selected provider/model with `context_window_tokens = Some(200000)`.
  - Assert selection carries `Some(200000)` into the runtime and later agent config/context policy.

- `select_runtime_missing_cache_context_window_returns_none`
  - Arrange valid config with no matching cache entry.
  - Assert runtime selection succeeds and carries `context_window_tokens = None`, relying on `ContextPolicy::default()` instead of failing.

- `agent_config_from_runtime_applies_reasoning_policy_and_context_window`
  - Drive a public helper such as `agent_config_for_runtime(&runtime, system_prompt)` if the implementation has one.
  - Assert `AgentConfig.model = Some(runtime.model)`, `include_reasoning = runtime.reasoning.non_none()`, `reasoning_effort = Some(runtime.reasoning.as_str())` for non-none reasoning, and `context_policy.model_context_window_tokens = runtime.context_window_tokens`.

- `agent_config_from_runtime_none_reasoning_disables_reasoning_request`
  - Arrange `ReasoningLevel::None`.
  - Assert `include_reasoning = false` and `reasoning_effort = None`.

- `provider_for_runtime_openrouter_api_key_constructs_openrouter_provider`
  - Arrange selected runtime with `provider_type = OPENROUTER_PROVIDER_ID` and API-key auth.
  - Assert provider construction succeeds through a public `llms` path and the resulting wrapper reports OpenRouter metadata through `LlmProvider::metadata()`.
  - Do not call live network model discovery or streaming.

- `provider_for_runtime_openai_api_key_constructs_openai_provider`
  - Arrange selected runtime with `provider_type = OPENAI_PROVIDER_ID` and API-key auth.
  - Assert provider construction succeeds with `OpenAiProvider::with_api_key_and_debug_logger` and metadata reports OpenAI.

- `provider_for_runtime_openai_oauth_constructs_openai_provider_with_worker_auth_store`
  - Arrange selected runtime with OAuth auth and an in-memory config I/O fake containing `ChatGptAuthConfig`.
  - Assert provider construction succeeds with `OpenAiProvider::with_debug_logger(Arc<WorkerOpenAiAuthStore>, ...)` and metadata reports OpenAI.
  - Also directly test the store load/save behavior below because the provider wrapper should not expose private auth internals.

- `worker_openai_auth_store_loads_oauth_from_named_provider`
  - Arrange fake config with provider OAuth credentials including token fields and account metadata.
  - Assert `load_openai_auth()` returns the equivalent `OpenAiAuthRecord`.

- `worker_openai_auth_store_save_persists_refreshed_oauth_to_named_provider`
  - Arrange fake config and save an `OpenAiAuthRecord` with changed access/refresh tokens and metadata.
  - Assert the fake writer receives a `DoricConfig` where `set_provider_oauth` stored equivalent `ChatGptAuthConfig` for the selected provider.

- `worker_openai_auth_store_missing_oauth_returns_authentication_required_without_tokens`
  - Arrange provider missing OAuth credentials.
  - Assert `ProviderError::AuthenticationRequired` or a mapped worker error, and assert the display/lifecycle message contains no token-like values.

- `provider_for_runtime_unsupported_provider_returns_lifecycle_ready_error`
  - Arrange `provider_type = "anthropic"` or another unregistered type.
  - Assert failure maps to a worker provider unsupported reason and a concise message naming the provider type but not dumping config.

- `select_runtime_missing_coding_model_returns_redacted_provider_configuration_error`
  - Arrange config with no coding task assignment.
  - Assert a worker error wraps or maps the config failure, `lifecycle_failure_reason()` is provider/config setup related, and no config JSON or secrets appear in `lifecycle_failure_message()`.

- `select_runtime_missing_credentials_returns_missing_credentials_error`
  - Arrange provider exists but has no credentials or an empty API key.
  - Assert the error maps from `ConfigError::MissingProviderApiKey` or equivalent into a worker credentials/setup reason.

- `provider_for_runtime_openrouter_oauth_returns_unsupported_auth_mode`
  - Arrange OpenRouter with OAuth auth.
  - Assert worker rejects unsupported auth mode before provider construction, with no token content in the lifecycle message.

- `provider_for_runtime_openai_missing_auth_returns_missing_credentials_error`
  - Arrange OpenAI with no API key and no OAuth config.
  - Assert missing credentials are rejected before live provider calls.

- `provider_for_runtime_uses_disabled_debug_logger_by_default`
  - Construct provider through the production default helper and assert no debug log path is created when the caller does not pass one. If internals are opaque, drive this through a `WorkerProviderDeps::default()` accessor or a helper that returns the selected logger path.

- `provider_setup_failure_redacts_api_keys_oauth_tokens_and_config_payloads`
  - Arrange failures containing `sk-test_secret_1234567890abcdef`, access tokens, refresh tokens, and full credential-bearing config snippets.
  - Assert display and lifecycle messages redact token-like values and do not contain raw `apikey`, `access_token`, or `refresh_token` values.

- `worker_manifest_does_not_depend_on_cli`
  - Read `packages/worker/Cargo.toml` as text or parse with `toml` if the test suite already has a parser.
  - Assert `[dependencies]` does not include `cli`.
  - This can be a unit test or a validation checklist item if adding TOML parsing just for this is too much.

- `workspace_does_not_extract_runtime_or_provider_runtime_package_for_effort_09`
  - Prefer validation with `cargo metadata --format-version 1 --no-deps` or a targeted file check.
  - Assert no workspace member/package named `runtime` or `provider-runtime` was added by effort 09.
  - This is acceptable as a validation command/check rather than a Rust unit test if the test writer keeps unit scope cleaner.

### Red command

Focused red command:

```text
cargo test -p worker provider --no-fail-fast
```

Expected initial failure mode:

- Before tests exist: Cargo reports no `provider` tests or the test writer first updates `packages/worker/tests/unit.rs` and then the command fails to compile because `worker::provider`, `WorkerRuntimeSelection`, `select_runtime`, `provider_for_runtime`, provider-related `WorkerError` variants, and config/llms/agent dependencies do not exist in `packages/worker/Cargo.toml`.
- After the first test is added: failure should be an unresolved import or missing public API, not a live network call, filesystem config lookup, or environment credential dependency.

### Green and regression commands

Focused green command:

```text
cargo test -p worker provider --no-fail-fast
```

Worker package regression:

```text
cargo test -p worker --no-fail-fast
cargo clippy -p worker --all-targets -- -D warnings
```

Adjacent package regression from effort contract:

```text
cargo test -p config --no-fail-fast
cargo test -p llms --no-fail-fast
cargo test -p agent --no-fail-fast
```

Workspace/package-boundary checks:

```text
cargo metadata --format-version 1 --no-deps
cargo fmt --all -- --check
```

Nx equivalents when local Nx dependencies are available:

```text
npx nx run worker:test
npx nx run worker:lint
```

### Test implementation notes

- Keep all automated tests offline and deterministic. Do not validate real API keys, run OAuth browser flow, fetch models, or stream completions.
- Prefer in-memory fakes for config I/O and auth-store persistence.
- Use `LlmProvider::metadata()` and simple construction behavior as the public proof for provider dispatch. Avoid asserting private fields of `OpenRouterProvider` or `OpenAiProvider`.
- Keep duplicated chat comparison only as context. Tests should not import `packages/cli/src/chat/*` or make `worker` depend on `cli`.
- Keep provider composition in `packages/worker/src/provider.rs`; do not move chat provider code and do not create a lower shared runtime package in this effort.
- Add worker `Cargo.toml` dependencies only for public APIs used by worker production code and tests: expected likely dependencies are `agent`, `config`, `llms`, and possibly `tokio` if provider tests need async helper types. Avoid adding `cli`.

## Files changed

- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/098_effort_09_test_planner.md`

## Blocking questions

- None blocking for the test writer. The one design choice to make during implementation is whether `WorkerRuntimeSelection` owns fields directly or exposes accessors; the tests should drive whichever shape gives a small public worker contract.

## Coordinator decision

accepted
