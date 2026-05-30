# Agent Receipt: effort 05 test planner

## Spawn proof

- Tool: multi_agent_v1.spawn_agent
- Agent type: worker
- Agent id: 019e7682-d258-7070-98b8-8bd5b87e753d
- Spawn result: completed; wrote this receipt at the assigned path
- Required agents row: `| development | efforts/05_daemon_registry_root.md | test planner | worker | agents/058_effort_05_test_planner.md | 019e7682-d258-7070-98b8-8bd5b87e753d | accepted |`

## Role

Doric development test planner for effort 05, translating `efforts/05_daemon_registry_root.md` acceptance criteria into focused daemon root and registry tests.

## Input artifacts

- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/efforts/05_daemon_registry_root.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/PRD.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/TDD.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/FEATURES.md`
- `.agents/skills/coding-conventions/SKILL.md`
- `.agents/skills/coding-conventions/references/implementation-standards.md`
- `.agents/skills/coding-conventions/references/sexy-rust.md`
- `packages/daemon/Cargo.toml`
- `packages/daemon/src/lib.rs`
- `packages/daemon/src/main.rs`
- `packages/daemon/project.json`
- `packages/lifecycle/src/identity.rs`
- `packages/lifecycle/src/status.rs`
- `packages/lifecycle/src/event.rs`
- `packages/lifecycle/src/redaction.rs`
- `packages/lifecycle/src/repo.rs`
- `packages/config/src/lib.rs`
- `packages/config/src/persistence.rs`

## Read ownership

- Daemon skeleton files under `packages/daemon`
- Lifecycle public types in `packages/lifecycle/src/{identity.rs,status.rs,event.rs,redaction.rs}`
- Lifecycle repo identity helper in `packages/lifecycle/src/repo.rs` because list summaries need redacted repo identity
- Config path API in `packages/config/src/lib.rs` and included `persistence.rs`
- Existing package test patterns under `packages/lifecycle/tests/unit` and `packages/config/tests/unit`

## Write ownership

- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/058_effort_05_test_planner.md`

## Coding conventions used

- `.agents/skills/coding-conventions/SKILL.md`: constrained the plan to behavior tests, Red-Green-Refactor, F.I.R.S.T., one logical concept per test, public contracts over private state, explicit dependency injection at boundaries, and no speculative abstractions.
- `references/implementation-standards.md`: constrained test placement to package-root `tests/`, with only a minimal Rust `#[cfg(test)] mod tests { include!(...) }` harness in `src/lib.rs`; also constrained tests to Arrange-Act-Assert and `MethodUnderTest_Scenario_ExpectedBehavior` naming adapted to Rust.
- `references/sexy-rust.md`: constrained the recommended daemon APIs toward type-driven inputs and flat `Result`/`Option` flow using `WorkerId`, `RequestId`, `RepoIdentity`, `WorkerStatus`, `WorkerEvent`, and `StreamEvent` rather than stringly typed registry state.

## Prompt summary

Plan only the tests for effort 05. Do not edit production or test code. Ground the plan in current daemon, lifecycle, and config APIs. The effort owns daemon-local root resolution and the in-memory registry/event model without binding a server or spawning workers.

## Output

### Current code facts that shape the tests

- `packages/daemon/src/lib.rs` is empty and `packages/daemon/Cargo.toml` has no dependencies yet.
- `packages/lifecycle` already exposes `WorkerId`, `RequestId`, `RepoIdentity`, `WorkerStatus`, `WorkerEvent`, and `StreamEvent`.
- `config::config_dir()` is public and returns the Doric platform config directory; the default worker root convention in this effort is `config::config_dir()?.join("workers")`.
- Existing Rust package tests use package-root `tests/unit/*.rs` files included from a minimal `#[cfg(test)]` harness in `src/lib.rs`.

### Test files to create or update

- Create `packages/daemon/tests/unit/root.rs`.
- Create `packages/daemon/tests/unit/registry.rs`.
- Update `packages/daemon/src/lib.rs` with a minimal test harness:

```rust
#[cfg(test)]
mod tests {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/unit/root.rs"));
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/unit/registry.rs"));
}
```

- Update `packages/daemon/Cargo.toml` to depend on `config` and `lifecycle`; add no external test dependency unless implementation explicitly chooses one. Temporary directories can be built with `std::env::temp_dir()` plus a unique suffix, following the current `config` test style.

### Root tests mapped to acceptance criteria

`packages/daemon/tests/unit/root.rs`

- `resolve_worker_root_explicit_directory_returns_canonical_root`
  - Covers AC-13 and FR-11.
  - Arrange a unique existing temp directory. Act with an explicit worker root. Assert the resolved root points to the existing directory and does not use the config default.

- `resolve_worker_root_without_override_uses_config_dir_workers`
  - Covers AC-13 and TDD root convention.
  - Prefer an injectable default-root provider such as `resolve_worker_root_with_default(None, || Ok(config_dir.join("workers")))` so the test is repeatable without mutating global environment. Assert the returned path is `<config_dir>/workers`.

- `validate_worker_root_missing_directory_returns_root_configuration_error`
  - Covers AC-13.
  - Arrange a path that does not exist. Act with root validation. Assert the error is a daemon root-configuration error, includes the path or root context, and does not imply worker/job execution.

- `validate_worker_root_file_instead_of_directory_returns_root_configuration_error`
  - Covers AC-13.
  - Arrange a temp file at the requested root. Assert validation rejects it with a clear root-configuration error.

- `validate_worker_root_inaccessible_directory_returns_root_configuration_error`
  - Covers AC-13 where the platform can create an unreadable directory deterministically.
  - On Windows, avoid brittle ACL mutation unless the implementation provides a fake filesystem/root checker seam. If no reliable unreadable fixture exists, cover this through an injected `RootFs`/validator test double returning `PermissionDenied`.

- `prepare_worker_layout_valid_root_creates_expected_child_directories`
  - Covers F-06 root/repo handling that is partially coupled to this effort.
  - If effort 05 introduces per-worker root helpers, assert `<root>/<worker-id>/repo`, `state`, `artifacts`, and `tool-output` layout paths are derived without spawning or cloning. If layout is deferred to effort 08, omit this test from effort 05.

### Registry tests mapped to acceptance criteria

`packages/daemon/tests/unit/registry.rs`

- `new_registry_empty_list_returns_empty_summaries`
  - Covers AC-5 and in-memory registry behavior.
  - Assert a fresh registry lists no workers and does not synthesize stale entries.

- `insert_worker_valid_record_lists_summary_with_required_fields`
  - Covers AC-6, AC-12, and effort criterion "List summaries include id, mode, redacted repo identity, status, current activity or terminal reason, and timing or ordering signal."
  - Arrange a worker with mode `feature`, credential-bearing repo URL, status `Accepted`, activity text, and an ordering signal. Assert list summary exposes id, mode, redacted `RepoIdentity`, status, activity/reason, and sequence or timestamp.

- `insert_worker_duplicate_id_rejects_record`
  - Covers daemon as lifecycle authority and prevents registry corruption.
  - Assert a second insert for the same `WorkerId` returns a duplicate/known-worker error.

- `update_status_known_worker_records_current_activity`
  - Covers AC-6 and AC-9.
  - Move a worker from accepted/running to failed with a reason. Assert list shows `WorkerStatus::Failed` and the concise terminal reason.

- `update_status_unknown_worker_returns_unknown_worker_error`
  - Covers AC-8 and AC-14.
  - Assert unknown ids are not silently inserted by updates and produce unknown/untracked wording.

- `append_event_allocates_monotonic_sequences_from_zero_or_one_consistently`
  - Covers AC-7 and event sequence allocation.
  - The exact first sequence can be implementation-defined, but the test should pin it. Recommendation: first event sequence `0` because stream replay requests start from sequence `0`.

- `replay_events_from_zero_returns_retained_events_in_order`
  - Covers AC-7 and replay-before-live semantics.
  - Append accepted, starting, repo preparation, prompt-agent, waiting, and succeeded events. Assert replay from `0` returns retained `WorkerEvent`s in sequence order.

- `replay_events_before_first_available_prepends_history_truncated`
  - Covers AC-7 and effort criterion "history-truncated marker."
  - Use a small ring capacity, append beyond capacity, request from `0`, and assert the first stream item is `StreamEvent::history_truncated(worker_id, 0, first_available_sequence)` before retained events.

- `replay_events_from_middle_returns_only_requested_suffix`
  - Covers event replay contract.
  - Assert requesting from sequence `n` returns events with sequence `>= n` and no truncation marker when `n` is still retained.

- `request_input_known_running_worker_marks_waiting_and_stores_pending_request`
  - Covers AC-11.
  - Arrange a running worker. Act with input request `(worker_id, request_id, prompt)`. Assert status becomes `WaitingForInput`, list summary exposes request text/correlation handle, and replay includes a `waiting_for_input` event with `RequestId`.

- `answer_input_pending_request_accepts_once_and_emits_continuation`
  - Covers AC-11.
  - Arrange pending input. Act with an answer. Assert success, pending entry is cleared, status returns to running/current activity, and a continuation/answer-received event is appended.

- `answer_input_unknown_worker_rejects_without_side_effects`
  - Covers AC-8 and AC-11.
  - Assert unknown worker answer returns unknown/untracked and no pending entry is created.

- `answer_input_duplicate_request_rejects_second_answer`
  - Covers AC-11 duplicate answer rejection.
  - First answer succeeds; second answer for the same `(worker_id, request_id)` fails with duplicate/stale/no-pending error.

- `answer_input_stale_request_rejects_old_request_after_new_wait`
  - Covers AC-11 stale answer rejection.
  - Arrange request `r1`, supersede it with request `r2` for the same worker, then answer `r1`. Assert stale request error and no command is forwarded.

- `answer_input_non_waiting_worker_rejects_answer`
  - Covers AC-11 non-waiting validation.
  - Arrange a running or terminal worker with no pending input. Assert answer returns non-waiting/no-pending error.

- `terminal_worker_rejects_new_input_and_preserves_terminal_reason`
  - Covers AC-6, AC-9, AC-14.
  - Arrange succeeded/failed/stopped worker. Assert input request or answer fails and list keeps terminal status/reason.

- `new_registry_after_previous_instance_has_no_workers`
  - Covers AC-14 and explicit in-memory behavior.
  - Insert worker in one registry, create a new registry instance, assert list is empty and stream/answer for old id returns unknown/untracked.

### Focused red commands and expected failure mode before implementation

Run these after adding only the planned daemon tests and the minimal `src/lib.rs` test harness:

```text
cargo test -p daemon --no-fail-fast root::
cargo test -p daemon --no-fail-fast registry::
```

Expected red failure before implementation:

- Compile failure, not assertion failure, because `daemon` currently exposes no `root`, `registry`, `event`, or `error` modules and has no `config`/`lifecycle` dependencies.
- Representative errors should be unresolved imports such as `could not find root in daemon`, `could not find registry in daemon`, unresolved `lifecycle` types from the daemon crate, or missing daemon error types.
- If a test compiles but fails, the expected first assertion failures should be contract-level: missing default root `config_dir()/workers`, missing clear root-configuration error, missing bounded event replay/truncation, or missing pending-input validation.

Avoid treating unrelated tool setup failures as valid Red. If Cargo cannot compile due to dependency metadata or lockfile state unrelated to daemon tests, fix the setup before counting the Red result.

### Regression commands to run after implementation

Focused effort gates:

```text
cargo fmt --all -- --check
cargo test -p daemon --no-fail-fast
cargo test -p lifecycle --no-fail-fast
cargo clippy -p daemon --all-targets -- -D warnings
npx nx run daemon:test
```

Useful fallback when Nx is unavailable or Windows target locks appear:

```text
$env:CARGO_TARGET_DIR='dist\target\effort-05-verify'; cargo test -p daemon --no-fail-fast
$env:CARGO_TARGET_DIR='dist\target\effort-05-verify'; cargo clippy -p daemon --all-targets -- -D warnings
```

Broader confidence before handoff to effort 06:

```text
cargo test -p lifecycle --no-fail-fast
cargo test -p daemon --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
```

### Testability risks and API shape recommendations

- Default worker-root testing will be brittle if `root` calls `config::config_dir()` directly with no seam. Recommendation: expose a small public or `pub(crate)` API that accepts an optional explicit root plus an injected default-root provider for tests; production can pass `|| config::config_dir().map(|path| path.join("workers"))`.
- Inaccessible-directory tests are platform-sensitive, especially on Windows. Recommendation: keep filesystem validation behind a tiny trait or checker function so permission failures can be tested deterministically without ACL manipulation.
- Registry tests should use lifecycle domain types directly rather than raw strings: `WorkerId`, `RequestId`, `RepoIdentity`, `WorkerStatus`, `WorkerEvent`, and `StreamEvent`.
- Event replay should return a typed stream item that can represent either a lifecycle event or `history_truncated`; otherwise tests will have to inspect private ring internals or parse messages.
- Pending input should be represented by `(WorkerId, RequestId)` and answer validation should return distinct errors for unknown worker, no pending request, stale request, duplicate answer, and non-waiting/terminal worker. Distinct variants make CLI error mapping in later efforts straightforward.
- List summaries need a stable ordering signal. Prefer exposing `last_sequence` or `created_sequence` in the summary for deterministic unit tests; wall-clock timestamps are harder to assert without injecting a clock.
- Keep this effort spawn-free. Tests should prove registry/root behavior with fake records and events only; process attach, worker sessions, and gRPC service behavior belong to later efforts.

## Files changed

- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/058_effort_05_test_planner.md`

## Blocking questions

- None. Default root, in-memory registry, replay-before-live, and pending-input behavior are sufficiently decided by the effort, PRD, TDD, and FEATURES artifacts.

## Coordinator decision

accepted
