# Agent Receipt: effort 09 test writer

## Spawn proof
- Tool: multi_agent_v1.spawn_agent
- Agent type: worker
- Agent id: 019e76f3-ba75-7db1-8563-aab135ef036b
- Spawn result: spawned
- Required agents row: `development | efforts/09_worker_provider_runtime.md | test writer | worker | agents/099_effort_09_test_writer.md`

## Role

Development test-writer sub-agent for effort 09. This role writes the worker provider/runtime tests before implementation, keeps the red failure focused on the missing worker-local provider API, and avoids production behavior changes.

## Input artifacts

- `.agents/skills/coding-conventions/SKILL.md`
- `.agents/skills/coding-conventions/references/implementation-standards.md`
- `.agents/skills/coding-conventions/references/sexy-rust.md`
- `.agents/skills/coding-conventions/references/monodon-rust.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/STATE.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/efforts/09_worker_provider_runtime.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/098_effort_09_test_planner.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/TDD.md`
- Worker current files listed in the spawn prompt.
- Read-only config, llms, agent, and cli runtime-selection context listed in the spawn prompt.

## Read ownership

- `packages/worker/Cargo.toml`
- `packages/worker/src/lib.rs`
- `packages/worker/src/error.rs`
- `packages/worker/tests/unit.rs`
- `packages/worker/tests/unit/repo.rs`
- `packages/config/src/lib.rs`
- `packages/config/src/schema.rs`
- `packages/config/src/provider_schema.rs`
- `packages/config/src/config_model.rs`
- `packages/config/src/errors.rs`
- `packages/llms/src/lib.rs`
- `packages/llms/src/provider.rs`
- `packages/llms/src/registry.rs`
- `packages/llms/src/openai/auth.rs`
- `packages/llms/src/openai/mod.rs`
- `packages/llms/src/openrouter/mod.rs`
- `packages/llms/src/types/error.rs`
- `packages/agent/src/lib.rs`
- `packages/agent/src/agent.rs`
- `packages/agent/src/context/policy.rs`
- `packages/cli/src/chat/runtime_selection.rs`
- `Cargo.toml`

## Write ownership

- `packages/worker/tests/unit/provider.rs`
- `packages/worker/tests/unit.rs`
- `packages/worker/Cargo.toml`
- `Cargo.lock`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/099_effort_09_test_writer.md`

## Coding conventions

- `SKILL.md`: Applied repository invariants for behavior-oriented tests, explicit dependency boundaries, and Red-Green-Refactor. The tests drive a worker-local provider API and avoid production implementation edits.
- `references/implementation-standards.md`: Applied package-root `tests/` placement, F.I.R.S.T., Arrange-Act-Assert, public-contract testing, dependency injection through fakes, and the 500-line hard limit. `packages/worker/tests/unit/provider.rs` is 403 lines after formatting.
- `references/sexy-rust.md`: Applied Rust-specific type-driven call sites, `Result`-returning seams, and flat helper structure. Tests use typed config/auth/runtime values instead of string-only assertions except where redaction is the behavior.
- `references/monodon-rust.md`: Applied Cargo package validation for the focused red command. The required command was run directly as `cargo test -p worker provider --no-fail-fast`.

## Prompt summary

Write effort 09 tests before implementation. Cover worker runtime selection from `DoricConfig` and `ModelCache`, agent config reasoning/context mapping, OpenRouter/OpenAI provider construction metadata without network calls, OpenAI OAuth store load/save through a fake config I/O seam, provider/setup failure mapping to redacted lifecycle-ready worker errors, and the package-boundary requirement that `worker` must not depend on `cli`.

## Output

Added `packages/worker/tests/unit/provider.rs` and included it from `packages/worker/tests/unit.rs`.

The tests expect this public worker-local contract:

- `worker::provider::select_runtime(&DoricConfig, &ModelCache)`
- `worker::provider::agent_config_for_runtime(&WorkerRuntimeSelection, system_prompt)`
- `worker::provider::provider_for_runtime(&WorkerRuntimeSelection, WorkerProviderDeps)`
- `worker::provider::WorkerRuntimeSelection`
- `worker::provider::WorkerProviderAuth`
- `worker::provider::WorkerConfigIo`
- `worker::provider::WorkerOpenAiAuthStore`
- `worker::provider::WorkerProviderDeps`
- Provider-related `WorkerFailureReason` values for configuration, credentials, unsupported provider, unsupported auth, and setup failure.

The tests stay offline by using configured API-key strings only for construction and metadata assertions, and by using an in-memory config I/O fake for OAuth persistence.

## Red evidence

Command:

```text
cargo test -p worker provider --no-fail-fast
```

Exit code: `1`

Meaningful failure summary:

```text
error[E0432]: unresolved import `worker::provider`
  --> packages\worker\tests\unit\provider.rs:12:13
   |
12 | use worker::provider::{
   |             ^^^^^^^^ could not find `provider` in `worker`

error: could not compile `worker` (test "unit") due to 1 previous error
```

This is the expected red for missing effort 09 worker provider public APIs. The failure is not caused by missing test dependencies, live network calls, config files, environment credentials, or OAuth browser setup.

## Files changed

- `packages/worker/tests/unit/provider.rs`
- `packages/worker/tests/unit.rs`
- `packages/worker/Cargo.toml`
- `Cargo.lock`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/099_effort_09_test_writer.md`

## Blocking questions

- None.

## Coordinator decision

accepted
