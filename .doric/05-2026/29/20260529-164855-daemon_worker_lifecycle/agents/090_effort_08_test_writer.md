# Agent Receipt: effort 08 test writer

## Spawn proof
- Tool: multi_agent_v1.spawn_agent
- Agent type: worker
- Agent id: 019e76dd-a6f5-7511-a2e3-ae49e8f72838
- Spawn result: spawned
- Required agents row: `development | efforts/08_worker_repo_preparation.md | test writer | worker | agents/090_effort_08_test_writer.md`

## Role

Development test-writer sub-agent for effort 08. This role wrote the public-contract worker repo preparation tests before implementation and captured red evidence without adding production behavior.

## Input artifacts

- `.agents/skills/coding-conventions/SKILL.md`
- `.agents/skills/coding-conventions/references/implementation-standards.md`
- `.agents/skills/coding-conventions/references/sexy-rust.md`
- `.agents/skills/coding-conventions/references/monodon-rust.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/STATE.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/efforts/08_worker_repo_preparation.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/089_effort_08_test_planner.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/TDD.md`
- `packages/worker/Cargo.toml`
- `packages/worker/src/lib.rs`
- `packages/worker/src/main.rs`
- `packages/daemon/src/lib.rs`
- `packages/daemon/tests/unit/root.rs`
- `packages/lifecycle/tests/unit/redaction.rs`
- `packages/lifecycle/src/redaction.rs`
- `packages/lifecycle/src/repo.rs`
- `packages/lifecycle/src/identity.rs`

## Read ownership

Read-only access was limited to the required Doric state, effort plan, test-planner receipt, TDD, worker skeleton, nearby daemon test harness, and lifecycle redaction/identity context.

## Write ownership

- `packages/worker/tests/unit/repo.rs`
- `packages/worker/tests/unit.rs`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/090_effort_08_test_writer.md`

## Coding conventions

- `SKILL.md`: Applied focused behavior tests, Red-Green-Refactor, explicit dependency injection, public-contract testing, context-driven naming except for scenario-style test names, and the 500-line file limit.
- `references/implementation-standards.md`: Placed tests under `packages/worker/tests/`, used an integration-test harness instead of inline production tests, kept tests fast/no-network/self-validating, used Arrange-Act-Assert, and isolated Git with a fake runner.
- `references/sexy-rust.md`: Shaped the expected API around Rust-native `Result`, error variants, accessors, strongly typed worker ids, and flat error mapping without hidden global state.
- `references/monodon-rust.md`: Used Cargo's worker test target shape and the required `cargo test -p worker` validation path.

## Prompt summary

Write effort 08 tests before implementation for worker repo layout and external Git clone preparation. Keep all production behavior unimplemented. Use a fake `GitCommandRunner`, cover layout, conflict and failure mapping, capture red evidence, and write exactly this receipt.

## Output

Added a Cargo integration-test harness at `packages/worker/tests/unit.rs` that includes `packages/worker/tests/unit/repo.rs`.

Added focused public-contract tests that expect:

- `worker::repo` to expose `prepare_worker_layout`, `prepare_worker_repo`, `WorkerLayout`, `PreparedRepo`, `PrepareRepoRequest`, `GitCommand`, `GitOutput`, and injected `GitCommandRunner`.
- `worker::state` to expose a parseable `WorkerId`.
- `worker::error` to expose `WorkerError`, `WorkerResult`, and stable `WorkerFailureReason` mapping.
- Worker layout creates and returns stable `repo/`, `state/`, `artifacts/`, and `tool-output/` directories under `<root>/<worker-id>/`.
- Repo preparation refuses a non-empty clone target, permits an empty layout-created repo directory, constructs `git clone -- <repo-url> <repo-dir>` through the injected runner, sets the worker directory as command cwd, and uses a timeout.
- Missing Git, nonzero Git failure, timeout, cancellation, invalid root, clone target conflict, stderr redaction, repo URL redaction, and lifecycle failure reason mapping are observable through public errors.

## Red evidence

Command:

```text
cargo test -p worker --no-fail-fast prepare_worker_repo_uses_injected_git_runner_constructs_external_clone_command
```

Exit code: `1`

Meaningful failure summary:

```text
error[E0432]: unresolved import `worker::error`
error[E0432]: unresolved import `worker::repo`
error[E0432]: unresolved import `worker::state`
```

This is the expected red for the worker skeleton: the worker public API modules/types/functions required by effort 08 do not exist yet.

## Files changed

- `packages/worker/tests/unit.rs`
- `packages/worker/tests/unit/repo.rs`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/090_effort_08_test_writer.md`

## Blocking questions

None.

## Coordinator decision

accepted
