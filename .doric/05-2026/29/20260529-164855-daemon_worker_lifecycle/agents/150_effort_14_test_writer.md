# Agent Receipt: Effort 14 Test Writer

## Spawn proof

- Tool: multi_agent_v1.spawn_agent
- Agent type: worker
- Agent id: 019e786b-0977-7b42-a147-96da866151be
- Spawn result: spawned
- Required agents row: `development | efforts/14_lifecycle_integration_smoke.md | test writer | worker | agents/150_effort_14_test_writer.md | 019e786b-0977-7b42-a147-96da866151be | spawned`

## Role

Effort 14 test writer. Add the first red integration smoke test from the accepted test plan, avoid implementation changes, and record red evidence.

## Input artifacts

- `.agents/skills/coding-conventions/SKILL.md`
- `.agents/skills/coding-conventions/references/implementation-standards.md`
- `.agents/skills/coding-conventions/references/sexy-rust.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/STATE.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/efforts/14_lifecycle_integration_smoke.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/149_effort_14_test_planner.md`
- `packages/daemon/tests/unit/worker_session.rs`
- `packages/daemon/tests/unit/service.rs`
- `packages/daemon/src/worker_session.rs`
- `packages/daemon/src/registry.rs`
- `packages/daemon/src/service.rs`
- `packages/daemon/src/event.rs`
- `packages/daemon/src/lib.rs`
- `packages/daemon/Cargo.toml`
- `packages/lifecycle/proto/doric/lifecycle/v1.proto`

## Read ownership

Read was limited to the required Doric artifacts, Rust testing conventions, daemon lifecycle/session tests, and focused daemon/lifecycle source seams needed to write the smoke test.

## Write ownership

- `packages/daemon/tests/lifecycle_service.rs`
- `packages/daemon/tests/integration/lifecycle_service.rs`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/150_effort_14_test_writer.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/validation/14_lifecycle_integration_smoke.md`

## Coding conventions

- Loaded `.agents/skills/coding-conventions/SKILL.md` before editing.
- Used `references/implementation-standards.md` for package `tests/` placement, red-first validation, public-contract testing, and focused test scope.
- Used `references/sexy-rust.md` for Rust formatting, flat setup, and readable helper boundaries.
- Added a Cargo-visible `tests/lifecycle_service.rs` harness because Cargo does not automatically run nested `tests/integration/*.rs` files.

## Prompt summary

Add the first red daemon lifecycle integration smoke test named `lifecycle_service_worker_frames_replay_prompt_milestones_without_collapsing_names`. The test must drive daemon/session public APIs, record worker event frames for `repo_preparation`, `prompt_agent_started`, `prompt_artifact_written`, and `prompt_agent_completed`, stream replay from sequence `0`, assert the exact milestone names appear in order, and leave production code untouched.

## Red evidence

- Command: `cargo test -p daemon lifecycle_service_worker_frames_replay_prompt_milestones_without_collapsing_names --no-fail-fast`
- Exit code: `1`
- Failure summary:

```text
assertion `left == right` failed
  left: []
 right: ["repo_preparation", "prompt_agent_started", "prompt_artifact_written", "prompt_agent_completed"]
error: test failed, to rerun pass `-p daemon --test lifecycle_service`
error: 1 target failed:
    `-p daemon --test lifecycle_service`
```

The failure is the expected red state. Worker session event recording currently collapses event names through status-derived registry events, so replay from sequence `0` does not retain the prompt lifecycle milestone names required by effort 14.

## Files changed

- `packages/daemon/tests/lifecycle_service.rs`
- `packages/daemon/tests/integration/lifecycle_service.rs`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/150_effort_14_test_writer.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/validation/14_lifecycle_integration_smoke.md`

## Blocking questions

None. The test compiles and fails for the expected behavior gap.

## Coordinator decision

Coordinator decision: accepted.
