# Agent Receipt: effort 11 production prompt runner repair

## Spawn proof

- Tool: multi_agent_v1.spawn_agent
- Agent type: worker
- Agent id: 019e772a-6bf3-7f40-b9bf-02f844394335
- Spawn result: spawned
- Required agents row: `development | efforts/11_worker_prompt_agent_runner.md | production prompt runner repair writer | worker | agents/116_effort_11_production_prompt_runner_repair.md`

## Role

Repair writer for effort 11 production prompt-agent composition. The role was to close the gap between the fakeable prompt runner seam and the TDD/acceptance requirement that a production prompt runner can be composed from worker-local provider selection, worker tool registration, and `agent::Agent` without depending on `cli` or making live provider calls in tests.

## Input artifacts

- `.agents/skills/coding-conventions/SKILL.md`
- `.agents/skills/coding-conventions/references/implementation-standards.md`
- `.agents/skills/coding-conventions/references/sexy-rust.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/efforts/11_worker_prompt_agent_runner.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/TDD.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/115_effort_11_code_writer.md`
- `packages/worker/src/agents/prompt.rs`
- `packages/worker/src/provider.rs`
- `packages/worker/src/tooling.rs`
- `packages/worker/src/event.rs`
- `packages/lifecycle/src/event.rs`
- `packages/agent/src/agent.rs`
- `packages/agent/src/agent/constructors.rs`
- `packages/agent/src/lib.rs`
- `packages/agent/src/tool.rs`
- `packages/worker/tests/unit/prompt_agent.rs`
- `packages/worker/tests/unit/event.rs`
- `packages/lifecycle/tests/unit/domain.rs`

## Read ownership

- Effort 11 acceptance and sequencing notes under `efforts/11_worker_prompt_agent_runner.md`.
- TDD worker prompt-agent execution, worker-local provider/runtime composition, package dependency direction, testability, and prompt-agent-only scope sections.
- Previous code writer receipt `agents/115_effort_11_code_writer.md` showing the fake runner seam and the missing production composition.
- Worker provider/runtime and tooling APIs.
- Worker prompt event mapping and lifecycle event constructors.
- Agent constructors and public tool/config/store APIs.

## Write ownership

- `packages/worker/src/agents/prompt.rs`
- `packages/worker/src/event.rs`
- `packages/worker/tests/unit/prompt_agent.rs`
- `packages/worker/tests/unit/event.rs`
- `packages/lifecycle/src/event.rs`
- `packages/lifecycle/tests/unit/domain.rs`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/116_effort_11_production_prompt_runner_repair.md`

## Coding conventions

- `SKILL.md`: Applied scoped Rust implementation, explicit dependency boundaries, existing-code reuse, cohesive files under the 500-line limit, and no speculative new abstraction beyond the production composition seam.
- `references/implementation-standards.md`: Kept tests under `packages/worker/tests/`, tested public contracts, used explicit dependency injection at the composition boundary, and preserved focused unit tests with no live network dependency.
- `references/sexy-rust.md`: Used flat `Result` propagation, immutable bindings by default, direct type-driven composition with `WorkerRuntimeSelection`, `WorkerProvider`, `WorkerLayout`, and `agent::Store`, plus rustfmt/clippy validation.

## Prompt summary

Add a small production builder seam in `worker::agents::prompt` proving a prompt agent can be built from worker-local provider composition and worker tool registration. The seam must compose `agent_config_for_runtime`, `worker_tool_storage`, and `agent::Agent::with_config_and_store(...).with_tools(...)`, without depending on `cli`, without executing a live LLM call, and without implementing the full session loop.

## Output

- Added `WorkerPromptAgent`, a small wrapper around `agent::Agent<WorkerProvider>` that retains the config snapshot for offline contract assertions.
- Added `prompt_agent_for_worker(provider, runtime, layout, store, system_prompt)` in `worker::agents::prompt`.
- The builder composes `worker::provider::agent_config_for_runtime(runtime, system_prompt)`, `worker::tooling::worker_tool_storage(layout)?`, and `agent::Agent::with_config_and_store(provider, config, store).with_tools(tools)`.
- Added an offline prompt-agent unit test that builds a concrete `WorkerProvider` with `provider_for_runtime`, maps model/reasoning/context policy through `agent_config_for_runtime`, registers shared worker tools, asserts no `cli` dependency in the worker manifest, and never calls a live provider API.
- Replaced the worker-local duplicate `worker::event::WorkerEvent` with `lifecycle::event::WorkerEvent` as the return type of `prompt_agent_event_to_worker_event`.
- Added narrow lifecycle constructors for `prompt_artifact_written`, `prompt_answer_consumed`, `prompt_agent_completed`, and `prompt_agent_failed`.
- Preserved lifecycle failure redaction for prompt-agent failures and request id correlation for `waiting_for_input` and `prompt_answer_consumed`.
- Added lifecycle constructor coverage for prompt artifact, answer correlation, completion, and prompt failure redaction.
- Kept fake prompt runner behavior unchanged and did not add a session loop, daemon execution, PRD/TDD/decomposition phase claims, or live LLM execution.
- File sizes after repair: `packages/worker/src/agents/prompt.rs` is 221 lines; `packages/worker/src/event.rs` is 31 lines; `packages/lifecycle/src/event.rs` is 239 lines; `packages/lifecycle/tests/unit/domain.rs` is 185 lines; `packages/worker/tests/unit/prompt_agent.rs` is 330 lines.

## Validation

- `cargo test -p worker prompt_agent --no-fail-fast`
  - Passed: 15 filtered tests.
- `cargo test -p worker event --no-fail-fast`
  - Passed: 5 filtered tests.
- `cargo test -p lifecycle --no-fail-fast`
  - Passed: 19 lifecycle tests.
- `cargo test -p worker --no-fail-fast`
  - Passed: 47 worker tests.
- `cargo clippy -p worker --all-targets -- -D warnings`
  - Passed.
- `cargo fmt --all -- --check`
  - Passed.

## Files changed

- `packages/worker/src/agents/prompt.rs`
- `packages/worker/src/event.rs`
- `packages/worker/tests/unit/prompt_agent.rs`
- `packages/lifecycle/src/event.rs`
- `packages/lifecycle/tests/unit/domain.rs`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/116_effort_11_production_prompt_runner_repair.md`

## Blocking questions

None.

## Coordinator decision

accepted
