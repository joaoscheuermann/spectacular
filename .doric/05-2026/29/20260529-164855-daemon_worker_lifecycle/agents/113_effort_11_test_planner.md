# Agent Receipt: effort 11 test planner

## Spawn proof
- Tool: multi_agent_v1.spawn_agent
- Agent type: worker
- Agent id: 019e7720-0c1d-7e52-bf7a-d58a9ea1e59f
- Spawn result: spawned
- Required agents row: `development | efforts/11_worker_prompt_agent_runner.md | test planner | worker | agents/113_effort_11_test_planner.md`

## Role

Development test-planner sub-agent for effort 11. This role translates the worker prompt-agent runner acceptance criteria into concrete tests, red/green commands, and review assertions. This receipt is planning-only and does not edit production source or test files.

## Input artifacts

- `.agents/skills/coding-conventions/SKILL.md`
- `.agents/skills/coding-conventions/references/implementation-standards.md`
- `.agents/skills/coding-conventions/references/sexy-rust.md`
- `.agents/skills/coding-conventions/references/architecture-principles.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/STATE.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/efforts/11_worker_prompt_agent_runner.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/TDD.md`
- `packages/worker/src/error.rs`
- `packages/worker/src/state.rs`
- `packages/worker/src/provider.rs`
- `packages/worker/src/tooling.rs`
- `packages/lifecycle/src/event.rs`
- `packages/agent/src/lib.rs`
- `packages/agent/src/agent.rs`
- `packages/agent/src/event.rs`
- `packages/agent/src/store.rs`
- `packages/agent/src/tool.rs`
- `packages/worker/tests/unit.rs`
- `packages/worker/tests/unit/provider.rs`
- `packages/worker/tests/unit/repo.rs`
- `packages/worker/tests/unit/tooling.rs`

## Read ownership

- Effort state and design context:
  - `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/STATE.md`
  - `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/efforts/11_worker_prompt_agent_runner.md`
  - `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/TDD.md`
- Worker package context:
  - `packages/worker/src/error.rs`
  - `packages/worker/src/state.rs`
  - `packages/worker/src/provider.rs`
  - `packages/worker/src/tooling.rs`
  - `packages/worker/src/lib.rs`
  - `packages/worker/tests/unit.rs`
  - `packages/worker/tests/unit/provider.rs`
  - `packages/worker/tests/unit/repo.rs`
  - `packages/worker/tests/unit/tooling.rs`
- Lifecycle event context:
  - `packages/lifecycle/src/event.rs`
- Agent runtime context:
  - `packages/agent/src/lib.rs`
  - `packages/agent/src/agent.rs`
  - `packages/agent/src/event.rs`
  - `packages/agent/src/store.rs`
  - `packages/agent/src/tool.rs`
- Local worktree context:
  - `git status --short`

## Write ownership

- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/113_effort_11_test_planner.md` only.

## Coding conventions

- `SKILL.md`: Applied repository invariants for scoped work, behavior-oriented tests, explicit dependency boundaries, reuse of existing worker/provider/tooling seams, and no speculative shared abstractions.
- `references/implementation-standards.md`: Applied package-root `tests/` placement, Red-Green-Refactor, F.I.R.S.T., Arrange-Act-Assert, public-contract testing, and explicit dependency injection. New tests should live under `packages/worker/tests/unit/prompt_agent.rs` and `packages/worker/tests/unit/event.rs`, included from `packages/worker/tests/unit.rs`.
- `references/sexy-rust.md`: Applied Rust guidance for typed prompt job/result/event contracts, flat `Result` propagation, fakeable runner dependencies, clear `Option`/`Result` handling, and tests through public APIs instead of private state.
- `references/architecture-principles.md`: Applied package-boundary and dependency-direction guidance. The worker prompt runner should depend on `agent`, `lifecycle`, existing worker provider composition, and worker tool registration; it must not depend on `cli` or `daemon`, and must not move agent or tool ownership into `worker`.

## Prompt summary

Plan tests for effort 11 worker prompt-agent execution. The worker should expose a fakeable prompt runner seam, emit prompt/requirements-only lifecycle events, write `PROMPT.md` under `<worker-root>/<id>/artifacts/`, route human input requests with request text and a daemon-correlatable handle, consume answers through the runner seam, map failures to lifecycle failure events, and avoid names or output that claim PRD, TDD, decomposition, implementation, tests, validation, or handover work completed.

## Planned tests

### Worker test files and harness

- Add `packages/worker/tests/unit/prompt_agent.rs`.
- Add `packages/worker/tests/unit/event.rs` if lifecycle event mapping is separated from the runner; otherwise keep event-name assertions in `prompt_agent.rs` and add `event.rs` only when a public `worker::event` module exists.
- Update `packages/worker/tests/unit.rs` with:

```rust
#[path = "unit/prompt_agent.rs"]
mod prompt_agent;

#[path = "unit/event.rs"]
mod event;
```

- Keep fake runners, fake sinks, temp roots, and answer queues local to the test file until a second worker test module needs them.
- Test through public worker APIs such as `worker::agents::prompt::{PromptAgentRunner, PromptJob, PromptAgentResult}` and `worker::event` mapping helpers. Avoid testing private fields or source text unless checking user-visible event/output scope.

### Public API shape to drive from tests

Drive a small worker-owned seam rather than a full session loop:

- `packages/worker/src/lib.rs` should expose `pub mod agents;` and `pub mod event;`.
- `packages/worker/src/agents/mod.rs` should expose `pub mod prompt;`.
- `packages/worker/src/agents/prompt.rs` should expose:
  - `PromptJob` with worker id, prompt text, worker layout or artifact root, mode where needed, and enough context to build prompt-only output.
  - `PromptAgentRunner` as a fakeable trait matching the TDD shape.
  - `PromptAgentEvent` or equivalent internal event values for started, artifact written, input requested, answer consumed, completed, and failed.
  - An answer input abstraction that carries both request text and a correlation handle such as `lifecycle::RequestId`.
  - A production runner builder that composes `agent::Agent` only through worker provider/tooling seams.
- `packages/worker/src/event.rs` should map prompt-agent internal events into `lifecycle::WorkerEvent` names without generic SDLC phase names.

### Fake prompt runner tests

- `run_prompt_agent_fake_runner_started_emits_prompt_agent_started`
  - Arrange a temp worker layout and a fake runner that emits a started event.
  - Act through the worker prompt-agent orchestration API.
  - Assert the first prompt event maps to lifecycle name `prompt_agent_started`, status `Running`, and a prompt/requirements-only message.

- `run_prompt_agent_fake_runner_artifact_written_emits_prompt_artifact_written`
  - Arrange a fake runner that reports an artifact write for `PROMPT.md`.
  - Assert lifecycle output includes `prompt_artifact_written` and the artifact path or message identifies only `artifacts/PROMPT.md`.

- `run_prompt_agent_fake_runner_input_requested_includes_request_text_and_correlation_handle`
  - Arrange a fake runner that requests human input with request text such as `Which repository constraints should the prompt include?` and a deterministic `RequestId`.
  - Assert the mapped lifecycle event has status `WaitingForInput`, name `waiting_for_input` or prompt-specific equivalent, the request text in the message, and `request_id()` equal to the deterministic correlation handle.

- `run_prompt_agent_fake_runner_answer_consumed_emits_answer_consumed`
  - Arrange a pending input and provide an answer through the prompt-runner answer seam.
  - Assert the runner consumes the answer associated with the same request id and emits `prompt_answer_consumed` or an equivalent prompt-scoped continuation event.
  - Assert the event does not expose unrelated daemon/session internals.

- `run_prompt_agent_fake_runner_completed_emits_prompt_agent_completed`
  - Arrange a fake successful result with prompt content.
  - Assert terminal prompt-agent lifecycle event name is `prompt_agent_completed`, status is `Succeeded` only at the worker lifecycle boundary if the runner owns terminal mapping, and output describes prompt/requirements completion only.

- `run_prompt_agent_fake_runner_failed_emits_prompt_agent_failed_with_redacted_message`
  - Arrange a fake failure containing token-like text.
  - Assert lifecycle event name is `prompt_agent_failed` or the worker failure event preserves a prompt-agent-scoped message, status is `Failed`, and secrets are redacted through existing lifecycle/worker error redaction behavior.

### Artifact tests

- `write_prompt_artifact_prepared_layout_writes_prompt_md_under_artifacts`
  - Arrange `prepare_worker_layout(root, worker_id)` from existing worker repo tests.
  - Act through the public prompt artifact helper or successful fake prompt run with content `# Product requirements`.
  - Assert the file exists at `<worker-root>/<id>/artifacts/PROMPT.md`.
  - Assert the same `PROMPT.md` is not created under `<worker-root>/<id>/repo/`, `state/`, or `tool-output/`.

- `write_prompt_artifact_existing_prompt_md_replaces_or_updates_prompt_artifact_deterministically`
  - Arrange an existing `artifacts/PROMPT.md`.
  - Act with new prompt content.
  - Assert the final file content exactly matches the new prompt artifact contract selected by implementation, without appending duplicate stale prompt sections.

### Prompt-only scope tests

- `prompt_agent_lifecycle_names_do_not_claim_later_doric_phases`
  - Collect every lifecycle event name emitted by the fake-runner success and failure paths.
  - Assert none contains `prd`, `tdd`, `technical_design`, `decomposition`, `implementation`, `tests`, `validation`, or `handover`.
  - Assert allowed names are prompt-scoped: `prompt_agent_started`, `prompt_artifact_written`, `waiting_for_input`, `prompt_answer_consumed`, `prompt_agent_completed`, and `prompt_agent_failed` or the exact final names selected by the event API.

- `prompt_agent_messages_do_not_claim_later_doric_phases_completed`
  - Collect user-visible event messages and final output strings.
  - Assert no message says or implies PRD, TDD, decomposition, implementation, tests, validation, or handover completed.
  - This test may allow words only when negating scope in static docs is not part of runtime output; runtime output should be prompt/requirements-only.

### Production agent composition tests

If effort 11 production runner directly composes `agent::Agent`, add a focused test in `packages/worker/tests/unit/prompt_agent.rs`:

- `production_prompt_runner_worker_runtime_selection_maps_agent_config_and_tools`
  - Arrange a `WorkerRuntimeSelection` using existing provider test helpers or local equivalents, a fake/no-network provider, and `worker::tooling::worker_tool_storage(&layout)`.
  - Act through the production prompt runner's agent factory seam.
  - Assert `AgentConfig.model`, `include_reasoning`, `reasoning_effort`, and `ContextPolicy.model_context_window_tokens` match `worker::provider::agent_config_for_runtime`.
  - Assert registered tool manifests match the worker tooling seam and the runner does not depend on `cli`.

If the production runner does not compose `agent::Agent` in effort 11, the exact later seam must be documented as `packages/worker/src/agents/prompt.rs::ProductionPromptAgentRunner` or its `AgentFactory`/builder method. That seam must receive `WorkerRuntimeSelection`, `worker::provider::provider_for_runtime`, `worker::provider::agent_config_for_runtime`, and `worker::tooling::worker_tool_storage` before effort 12 wires the worker session loop. Do not defer config/reasoning/context-policy mapping to daemon or CLI code.

The existing `packages/worker/tests/unit/provider.rs` already covers `agent_config_for_runtime` model, reasoning, and context-window mapping through the worker provider seam. Effort 11 only needs an additional composition test if the new production prompt runner owns an `agent::Agent` construction path.

## Commands

Focused red command before implementation, after the test writer adds the first prompt-agent tests:

```text
cargo test -p worker prompt_agent --no-fail-fast
```

Expected initial failure mode:

- Compile failure because `worker::agents::prompt`, `worker::event`, prompt artifact helpers, or prompt answer/request types do not exist yet; or
- Assertion failure showing missing prompt-agent event mapping, missing `PROMPT.md` artifact write, missing request id on input, or later-phase names in runtime output.

The red must not fail because of live LLM calls, live provider credentials, network repo access, daemon process startup, or mutation outside the temp worker root.

Focused green commands:

```text
cargo test -p worker prompt_agent --no-fail-fast
cargo test -p worker event --no-fail-fast
cargo test -p worker --no-fail-fast
```

Green regression commands from the effort file:

```text
cargo test -p worker --no-fail-fast
cargo test -p agent --no-fail-fast
cargo test -p lifecycle --no-fail-fast
cargo clippy -p worker --all-targets -- -D warnings
```

Workspace hygiene commands for the validator/refactor pass:

```text
cargo fmt --all -- --check
git diff -- packages/worker/src packages/worker/tests packages/lifecycle/src packages/agent/src
```

Expected review results:

- Worker tests use fake prompt runners and no live LLM/network repo dependencies.
- `PROMPT.md` writes only under `artifacts/`.
- Runtime event names and output are prompt/requirements-only.
- Any `agent::Agent` construction is fed by worker provider/tooling seams, not `cli`.
- No source or tests outside the effort 11 write scope are required for the prompt-agent runner plan.

## Files changed

- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/113_effort_11_test_planner.md`

## Blocking questions

- None for test planning.

## Coordinator decision

accepted
