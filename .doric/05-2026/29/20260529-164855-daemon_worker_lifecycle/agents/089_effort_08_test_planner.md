# Agent Receipt: effort 08 test planner

## Spawn proof
- Tool: multi_agent_v1.spawn_agent
- Agent type: worker
- Agent id: 019e76db-2df5-7f33-84ad-81ddd0934604
- Spawn result: spawned
- Required agents row: `development | efforts/08_worker_repo_preparation.md | test planner | worker | agents/089_effort_08_test_planner.md`

## Role

Development test-planner sub-agent for effort 08. This role translates the worker repo preparation acceptance criteria into concrete tests, expected red failures, and validation commands for the worker repo preparation implementation. This role is planning-only and did not edit production code or test code.

## Input artifacts

- `.agents/skills/coding-conventions/SKILL.md`
- `.agents/skills/coding-conventions/references/implementation-standards.md`
- `.agents/skills/coding-conventions/references/monodon-rust.md`
- `.agents/skills/coding-conventions/references/sexy-rust.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/STATE.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/efforts/08_worker_repo_preparation.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/TDD.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/FEATURES.md`
- `packages/worker/Cargo.toml`
- `packages/worker/src/lib.rs`
- `packages/worker/src/main.rs`
- `packages/lifecycle/src/redaction.rs`
- `packages/daemon/src/root.rs`
- `packages/tools/src/path.rs`
- `packages/daemon/tests/unit/root.rs`
- `packages/lifecycle/tests/unit/redaction.rs`
- `packages/daemon/src/error.rs`
- `packages/worker/project.json`

## Read ownership

Read-only access was limited to the required artifacts, worker package skeleton, read-only coupled source files, and nearby existing Rust tests/errors needed to match repository conventions. Existing evidence shows `packages/worker` is still a skeleton: `Cargo.toml` has no dependencies, `src/lib.rs` is empty, and `src/main.rs` only contains `fn main() {}`.

## Write ownership

Wrote only `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/089_effort_08_test_planner.md`.

## Coding conventions

- `SKILL.md`: Applied repository invariants for explicit dependency injection, focused behavior tests, Red-Green-Refactor, context-driven naming, no speculative abstractions, and Rust-only guidance only for Rust files.
- `references/implementation-standards.md`: Constrains effort 08 tests to `packages/worker/tests/unit/repo.rs`, with no inline production tests except a minimal `#[cfg(test)] mod tests { include!(...) }` harness if needed. Tests must be fast, isolated, repeatable, self-validating, behavior-oriented, and use Arrange-Act-Assert. Public APIs should receive dependencies explicitly, which makes the Git runner injectable.
- `references/monodon-rust.md`: Confirms `worker` should be validated through the existing `@monodon/rust` target shape and `dist/target/worker`; focused Nx validation is `npx nx run worker:test` when local Nx tooling is available.
- `references/sexy-rust.md`: Constrains the recommended API shape to Rust-native `Result`/enum error modeling, flat `?` propagation, type-driven boundaries for worker layout and repo preparation, clear iterator or loop choices, and `rustfmt`/`clippy` compliance.

## Prompt summary

Plan effort 08 tests for worker-local root layout and repo preparation using an injected external Git CLI runner. Do not edit production or test code. The plan must cover layout creation, invalid roots, clone target conflicts, fake/no-network Git execution, missing Git, nonzero exit, timeout, cancellation, stderr/repo URL redaction, and lifecycle failure-event mapping.

## Output

Recommended public API shape for the test writer to create because `packages/worker` has no API yet:

- Expose modules from `packages/worker/src/lib.rs`: `pub mod error; pub mod repo; pub mod state;`.
- Add `repo::WorkerLayout` with `worker_root()`, `repo()`, `state()`, `artifacts()`, and `tool_output()` accessors. Keep child names exactly `repo`, `state`, `artifacts`, and `tool-output`.
- Add `repo::prepare_worker_repo(root: impl AsRef<Path>, worker_id: &WorkerId, repo_url: &str, runner: &dyn GitCommandRunner) -> WorkerResult<PreparedRepo>` or an async equivalent if the runner needs timeout/cancellation integration.
- Add `repo::prepare_worker_layout(root: impl AsRef<Path>, worker_id: &WorkerId) -> WorkerResult<WorkerLayout>` if layout creation is separable from clone execution.
- Add `repo::GitCommandRunner` as an injected seam, not a concrete global call. A simple testable shape is `fn run(&self, command: GitCommand) -> WorkerResult<GitOutput>`, where `GitCommand` exposes `program`, `args`, `cwd`, and optional timeout metadata for assertions.
- Add `repo::GitOutput { status: ExitStatusLike, stderr: String }` or a small local status enum so fake tests do not need to construct platform-specific `std::process::ExitStatus`.
- Add `error::WorkerError` variants for `InvalidWorkerRoot`, `CloneTargetConflict`, `GitMissing`, `GitFailed`, `GitTimeout`, `GitCancelled`, and `Redaction`. Each variant that can include Git stderr or repo URL text must expose only redacted display text.
- Add a public mapping such as `WorkerError::to_lifecycle_failure_reason()` or `event::worker_failure_from_error(worker_id, error)` when effort 08 introduces event mapping. If event mapping is deferred to a later worker event module, tests should still require a stable redacted failure reason enum/string that lifecycle code can consume.

Concrete tests to add in `packages/worker/tests/unit/repo.rs`:

- `prepare_worker_layout_valid_root_creates_repo_state_artifacts_and_tool_output_directories`: arrange a temp worker root, call layout preparation, assert all four directories exist under `<root>/<worker-id>/`.
- `prepare_worker_layout_valid_root_returns_stable_child_paths`: assert returned accessors equal `<root>/<worker-id>/repo`, `state`, `artifacts`, and `tool-output`.
- `prepare_worker_layout_missing_root_returns_invalid_worker_root`: arrange a missing root and assert no child paths are created and the error maps to root configuration/lifecycle failure text.
- `prepare_worker_layout_file_root_returns_invalid_worker_root`: arrange a file where the root directory should be and assert an invalid-root error with a clear root-config message.
- `prepare_worker_repo_existing_nonempty_repo_dir_returns_clone_target_conflict`: create `<root>/<worker-id>/repo` with a sentinel file before clone and assert the implementation refuses to clone into it.
- `prepare_worker_repo_existing_empty_repo_dir_policy_is_explicit`: choose and lock the policy before implementation. Preferred policy: layout may create `repo/`, and clone is allowed only if the directory is empty. Assert the fake runner receives `git clone <repo-url> <repo-dir>` and no mutation outside the worker root.
- `prepare_worker_repo_uses_injected_git_runner_constructs_external_clone_command`: fake runner records one command; assert `program == "git"`, args are `["clone", "--", raw_repo_url, repo_dir]` or another explicitly chosen safe arg shape, cwd is either the worker root or `None`, and no `git2` dependency or API is used.
- `prepare_worker_repo_fake_runner_success_never_uses_network`: fake runner returns success without spawning a process; assert layout and returned `PreparedRepo` are complete. This is the main no-network test.
- `prepare_worker_repo_missing_git_returns_git_missing_failure`: fake runner returns a not-found error; assert `WorkerError::GitMissing`, redacted display text, and lifecycle failure mapping.
- `prepare_worker_repo_git_nonzero_returns_git_failed_with_redacted_stderr`: fake runner returns nonzero plus stderr containing credentials and API-key-like text; assert the raw secret is absent from `Display`, debug-safe public reason, and lifecycle failure reason.
- `prepare_worker_repo_git_timeout_returns_git_timeout_failure`: fake runner returns timeout; assert timeout is distinct from generic nonzero failure and maps to a lifecycle failure event.
- `prepare_worker_repo_git_cancellation_returns_git_cancelled_failure`: fake runner returns cancellation; assert cancellation is distinct from timeout and nonzero failure and maps to lifecycle failure.
- `prepare_worker_repo_redacts_credential_bearing_repo_url_in_failure`: use `https://user:pass@github.com/org/repo.git`; assert failures mention `https://github.com/org/repo.git` and never `user`, `pass`, or `@github` userinfo.
- `prepare_worker_repo_redacts_clone_stderr_before_mapping_failure_event`: stderr should run through `lifecycle::redaction::redact_failure_text`, and repo URL text should run through `lifecycle::redaction::redact_repo_url` or `RepoIdentity::from_raw_url`.
- `worker_error_failure_variants_map_to_lifecycle_failure_reasons`: table-test `InvalidWorkerRoot`, `CloneTargetConflict`, `GitMissing`, `GitFailed`, `GitTimeout`, and `GitCancelled`; assert every variant has a stable, redacted, user-visible failure reason suitable for daemon stream/list events.

Implementation constraints the tests should enforce:

- Do not add `git2` or another Rust Git library. The test writer should inspect `packages/worker/Cargo.toml` in a small manifest test or rely on code review plus `cargo tree -p worker` after implementation. If a manifest test is added, keep it local and deterministic.
- Do not shell out in unit tests. All clone behavior must pass through a fake `GitCommandRunner`.
- Do not mutate the user's checkout or any path outside the temp worker root. Temp roots should use a unique directory under `std::env::temp_dir()` or a dev-dependency such as `tempfile`.
- Reuse lifecycle redaction behavior rather than duplicating ad hoc secret filtering in worker tests.
- Match existing Rust test style: package-level tests under `tests/unit`, optional source `include!` harness, scenario-focused snake_case names, and direct assertions on public behavior.

Focused red command:

```text
cargo test -p worker --no-fail-fast prepare_worker_repo_uses_injected_git_runner_constructs_external_clone_command
```

If the test writer creates a `tests/unit.rs` harness that includes `tests/unit/repo.rs`, this narrower form is also acceptable:

```text
cargo test -p worker --test unit --no-fail-fast prepare_worker_repo_uses_injected_git_runner_constructs_external_clone_command
```

Expected initial failure mode:

- The first red should fail at compile time because `worker::repo`, `worker::state`, `worker::error`, `WorkerError`, `WorkerLayout`, `PreparedRepo`, and `GitCommandRunner` do not exist yet. That is the correct red failure for this skeleton package.
- If the harness itself fails before reaching missing worker APIs, fix only the harness/import path until the failure is the missing public contract.

Green and regression commands:

```text
cargo test -p worker --no-fail-fast
cargo test -p lifecycle --no-fail-fast
cargo clippy -p worker --all-targets -- -D warnings
npx nx run worker:test
```

Broader regression after worker repo preparation is green:

```text
cargo fmt --all -- --check
cargo test -p daemon --no-fail-fast
cargo test -p tools --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
cargo build -p worker --bin doric-worker
```

## Files changed

- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/089_effort_08_test_planner.md`

## Blocking questions

- None for test planning.
- The test writer must choose and document the empty `repo/` clone target policy before implementation. Recommended policy: conflict only when `repo/` exists and is non-empty, because layout creation itself creates `repo/`.
- The exact lifecycle failure event type may land in a later worker event module. Effort 08 should still expose a stable redacted failure reason API so daemon/lifecycle mapping is mechanical.

## Coordinator decision

accepted
