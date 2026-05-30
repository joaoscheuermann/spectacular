# Effort: worker provider runtime

Status: done

## Requirement links

- Features: F-07, F-11, F-12
- PRD: FR-8, FR-10, FR-12, FR-13; AC-7, AC-10, AC-12, AC-15
- TDD: worker provider/runtime composition, Provider/runtime composition tournament, worker-local provider module responsibilities

## Goal

Implement worker-local provider/runtime composition from public `config`, `llms`, and `agent` APIs without depending on `cli` or extracting a lower runtime package.

## Sequence

- Position: 09 of 15
- Previous effort: 08_worker_repo_preparation.md
- Enables: prompt-agent runner can build a real provider/runtime path while keeping chat composition stable.

## Target files

- `packages/worker/Cargo.toml`
- `packages/worker/src/lib.rs`
- `packages/worker/src/provider.rs`
- `packages/worker/src/error.rs`
- `packages/worker/tests/unit/provider.rs`
- `Cargo.lock`

## Coupled files

- `packages/config/src/lib.rs`, `schema.rs`, `provider_schema.rs`, and `config_model.rs` expose the configuration APIs.
- `packages/llms/src/lib.rs`, `openai/auth.rs`, `openai/mod.rs`, `openrouter/mod.rs`, and `debug_log.rs` expose provider APIs.
- `packages/agent/src/agent.rs`, `context.rs`, and `store.rs` are read-only context for agent config and context policy.
- `packages/cli/src/chat/runtime_selection.rs`, `provider.rs`, and `auth.rs` are read-only comparison points, not dependencies.

## Ownership

- Intended worker write scope: worker provider module, provider tests, worker manifest updates, and local redacted provider error mapping.
- Read-only context: config, llms, agent, and cli chat provider composition.
- Known conflict risks: provider composition can drift from chat. Keep the duplicated boundary small and test every selected config/provider branch.

## Tests to add or update

- Coding-model selection tests from `DoricConfig` and `ModelCache`.
- Cached context-window fallback tests.
- OpenRouter and OpenAI API-key provider construction tests using fakeable seams where needed.
- OpenAI OAuth load/save tests through worker-local auth store.
- Unsupported provider, missing credentials, unsupported auth, reasoning policy, and disabled debug logger default tests.
- Redacted provider setup failure tests.

## Regression suites

- `cargo test -p worker --no-fail-fast`
- `cargo test -p config --no-fail-fast`
- `cargo test -p llms --no-fail-fast`
- `cargo test -p agent --no-fail-fast`
- `cargo clippy -p worker --all-targets -- -D warnings`

## Acceptance criteria

- Worker runtime selection uses `config::read_config_or_default`, `config::read_model_cache_or_default`, task model assignment, provider lookup, auth mode, API key, OAuth config, and cached model metadata.
- Worker provider construction uses public `llms` APIs for OpenRouter and OpenAI.
- Worker does not depend on `cli`.
- No new `runtime` or `provider-runtime` package is extracted in v1.
- Provider/config failures become redacted lifecycle-ready worker errors.

## Notes

- Preserve the repaired TDD decision: worker-local provider/runtime composition is intentional temporary duplication until chat and worker prove a shared lower API shape.
- Keep chat provider/runtime modules untouched.
