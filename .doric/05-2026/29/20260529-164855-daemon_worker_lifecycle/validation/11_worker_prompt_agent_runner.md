# Validation: worker prompt agent runner

## Red evidence

- `cargo test -p worker prompt_agent --no-fail-fast`
  - Exit code: 1
  - Expected red failure:
    - `error[E0433]: failed to resolve: could not find agents in worker` at `packages\worker\tests\unit\prompt_agent.rs:7:13`
    - `error[E0433]: failed to resolve: could not find agents in worker` at `packages\worker\tests\unit\event.rs:3:13`
    - `error[E0432]: unresolved import worker::event` at `packages\worker\tests\unit\event.rs:4:13`
- `cargo test -p worker event --no-fail-fast`
  - Exit code: 1
  - Expected red failure:
    - `error[E0433]: failed to resolve: could not find agents in worker` at `packages\worker\tests\unit\prompt_agent.rs:7:13`
    - `error[E0433]: failed to resolve: could not find agents in worker` at `packages\worker\tests\unit\event.rs:3:13`
    - `error[E0432]: unresolved import worker::event` at `packages\worker\tests\unit\event.rs:4:13`

## Green evidence

- `cargo test -p worker prompt_agent --no-fail-fast`
  - Exit code: 0
  - Passed: 15 prompt-agent/event filtered tests.
  - Confirms fake prompt runners run offline, prompt artifact behavior is deterministic, human input request events preserve request text and request id, answer-consumed events preserve request id, event names/messages remain prompt/requirements-only, and production prompt-agent composition builds offline.
- `cargo test -p worker event --no-fail-fast`
  - Exit code: 0
  - Passed: 5 filtered tests.
  - Confirms worker prompt events map to lifecycle `WorkerEvent` values and preserve request correlation.
- `cargo test -p worker --no-fail-fast`
  - Exit code: 0
  - Passed: 47 worker tests.
- `cargo test -p lifecycle --no-fail-fast`
  - Exit code: 0
  - Passed: 19 lifecycle tests.
- `cargo clippy -p worker --all-targets -- -D warnings`
  - Exit code: 0
  - Passed with no warnings.
- `cargo fmt --all -- --check`
  - Exit code: 0
  - Formatting check passed.
- `cargo metadata --format-version 1 --no-deps`
  - Exit code: 0
  - Metadata resolved successfully. Worker package dependencies are `agent`, `config`, `lifecycle`, `llms`, `tools`, and dev-only `tokio`; no `cli` dependency.
- `npx nx test worker`
  - Exit code: 0
  - Passed Nx worker test target; Cargo worker tests passed with 47 tests.
- `npx nx lint worker`
  - Exit code: 0
  - Passed Nx worker lint target; Cargo clippy target completed.

## Static validation

- Production builder: `packages/worker/src/agents/prompt.rs` composes `agent_config_for_runtime(runtime, system_prompt)`, `worker_tool_storage(layout)?`, and `agent::Agent::with_config_and_store(provider, config.clone(), store).with_tools(tools)`.
- CLI boundary: `packages/worker/Cargo.toml` has no `cli` dependency, and worker package metadata confirms the dependency boundary.
- Offline tests: production composition test uses `provider_for_runtime(..., WorkerProviderDeps::offline())` and asserts agent config/tool manifests without calling a live provider API.
- Lifecycle event mapping: `worker::event::prompt_agent_event_to_worker_event` returns `lifecycle::event::WorkerEvent`; no worker-local `WorkerEvent` type is defined.
- Prompt/requirements-only scope: runtime source has no PRD/TDD/decomposition/implementation/tests/validation/handover runtime event claims; later-phase terms appear only in test forbidden-word lists.

## Focused commands

- `cargo test -p worker prompt_agent --no-fail-fast`
- `cargo test -p worker event --no-fail-fast`

## Regression commands

- `cargo test -p worker --no-fail-fast`
- `cargo test -p lifecycle --no-fail-fast`
- `cargo clippy -p worker --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `cargo metadata --format-version 1 --no-deps`
- `npx nx test worker`
- `npx nx lint worker`

## Unavailable tooling

None. Nx was available and both required Nx targets passed.

## Refactors applied

- Fixed the stale `PromptAgentRunner` trait comment in `packages/worker/src/agents/prompt.rs`; it no longer says production composition is intentionally outside the seam after the production builder repair added `prompt_agent_for_worker`.

## Reviewer decision

accepted
