# Effort 07 Test Planner Receipt

## Spawn proof

| Field | Value |
| --- | --- |
| Tool | `multi_agent_v1.spawn_agent` |
| Agent type | `worker` |
| Agent id | `019e76b2-f410-7763-ba46-f0340308f2a6` |
| Required agents row status | `spawned` |

## Role

Effort 07 test planner for `efforts/07_daemon_process_worker_session.md`.

## Input artifacts

- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/STATE.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/efforts/07_daemon_process_worker_session.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/PRD.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/TDD.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/FEATURES.md`
- `.agents/skills/coding-conventions/SKILL.md`
- Existing daemon/lifecycle/worker seams under `packages/daemon/src/**`, `packages/daemon/tests/unit/**`, `packages/lifecycle/**`, and `packages/worker/src/main.rs`

## Read ownership

Read scope honored:

- Run state, effort, PRD, TDD, and FEATURES files listed above.
- Coding conventions entrypoint and Rust/test-planning references listed below.
- Existing daemon/lifecycle/worker files needed to identify feasible seams.

## Write ownership

Write scope honored:

- Wrote only `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/077_effort_07_test_planner.md`.

## Coding conventions used

- `.agents/skills/coding-conventions/SKILL.md`: testing should be TDD-driven, behavior-oriented, fast, isolated, repeatable, self-validating, and named with scenario intent.
- `.agents/skills/coding-conventions/references/implementation-standards.md`: tests belong under package `tests/`; Rust source may use only a minimal `#[cfg(test)] mod tests { include!(...) }` harness; dependencies should be injected at boundaries for fakes.
- `.agents/skills/coding-conventions/references/architecture-principles.md`: keep daemon policy independent from low-level process details through explicit boundaries; avoid speculative shared abstractions.
- `.agents/skills/coding-conventions/references/sexy-rust.md`: design Rust APIs around strong boundary types, flat error handling, and clear `Result`-returning seams.

## Current seam observations

- `packages/daemon/src/process.rs`, `packages/daemon/src/worker_session.rs`, `packages/daemon/tests/unit/process.rs`, and `packages/daemon/tests/unit/worker_session.rs` do not exist yet.
- `packages/daemon/src/lib.rs` currently includes unit test files for `root`, `registry`, `service`, and `server`; effort 07 tests should extend that include harness with `process` and `worker_session`.
- `packages/daemon/src/service.rs` already has injected `WorkerLauncher`, `CommandSender`, and `IdGenerator` fakes and passes `LaunchRequest` to a launcher, but it currently includes raw prompt/repo in `LaunchRequest`. Effort 07 process tests should prove the real process command does not pass raw prompt/repo as process arguments.
- `packages/lifecycle/proto/doric/lifecycle/v1.proto` already defines `WorkerSessionService`, `WorkerHello`, `StartJob`, `AnswerInputCommand`, and `ShutdownWorker`; worker-session tests should use this contract rather than inventing a CLI-to-worker protocol.
- `packages/worker/src/main.rs` is still an empty `fn main() {}` and must stay read-only for this effort planner. Tests that require real worker runtime behavior should be deferred.

## Proposed tests

### `packages/daemon/tests/unit/process.rs`

- `resolve_worker_binary_explicit_path_returns_configured_executable`
  - Intent: proves a configured worker binary path is accepted and used exactly when it exists.
  - Expected initial failure mode: `process` module and resolver API do not exist.

- `resolve_worker_binary_missing_configured_path_returns_configuration_error`
  - Intent: proves missing explicit `doric-worker` path fails before spawn with a clear configuration error.
  - Expected initial failure mode: resolver API does not exist.

- `resolve_worker_binary_without_config_uses_current_exe_sibling`
  - Intent: proves default lookup is based on the daemon/current executable directory sibling named `doric-worker` or platform executable equivalent.
  - Expected initial failure mode: resolver API does not exist.

- `build_worker_command_valid_launch_uses_worker_id_daemon_addr_token_and_worker_root`
  - Intent: proves command construction passes only process/session bootstrap data: daemon address, worker id, one-time token, and worker root/layout paths.
  - Expected initial failure mode: command builder and token-bearing launch request do not exist.

- `build_worker_command_valid_launch_omits_raw_prompt_repo_and_repo_identity`
  - Intent: protects daemon-controlled gRPC payload delivery by asserting prompt, raw repo URL, redacted repo identity, and other job payload fields are absent from process args.
  - Expected initial failure mode: current `LaunchRequest` still carries prompt/repo and no process command builder exists.

- `spawn_worker_success_records_child_handle_without_printing_token`
  - Intent: with a fake command runner/spawner, proves launch records child identity/liveness while token remains internal and never appears in display/loggable command output.
  - Expected initial failure mode: no fakeable child-process spawner trait exists.

- `spawn_worker_spawn_error_marks_worker_failed_with_redacted_reason`
  - Intent: proves spawn failures are mapped into daemon-visible failed state/events without leaking token or raw secret-bearing repo data.
  - Expected initial failure mode: no process-to-registry failure mapping exists.

- `monitor_child_exit_success_after_terminal_frame_keeps_succeeded`
  - Intent: proves a zero child exit after the worker already sent a terminal succeeded frame does not overwrite success.
  - Expected initial failure mode: no child monitor API exists.

- `monitor_child_exit_nonzero_without_terminal_frame_marks_failed`
  - Intent: proves a nonzero child exit before terminal worker frame becomes `failed` with a concise redacted reason.
  - Expected initial failure mode: no child exit mapper exists.

- `monitor_child_exit_signal_or_unavailable_without_terminal_frame_marks_stopped_or_unavailable`
  - Intent: proves stopped/unavailable process outcomes map to a daemon-visible stopped/unavailable style terminal state instead of remaining running.
  - Expected initial failure mode: no child exit mapper exists; lifecycle proto has no explicit unavailable enum, so the implementation may map unavailable internally and render compatible public status.

- `shutdown_attached_worker_sends_shutdown_then_terminates_child_on_timeout`
  - Intent: with fakes, proves graceful daemon shutdown first sends a daemon-frame shutdown and then best-effort terminates a still-running child.
  - Expected initial failure mode: no shutdown coordinator API or worker command channel exists.

- `shutdown_unattached_worker_terminates_child_without_worker_command`
  - Intent: proves workers that never attached are still cleaned up through the child process handle.
  - Expected initial failure mode: no child lifecycle tracker exists.

### `packages/daemon/tests/unit/worker_session.rs`

- `attach_worker_valid_hello_claims_token_once_and_sends_start_job`
  - Intent: proves worker-initiated stream validates worker id and one-time token, claims the token, transitions accepted/starting worker to running or attached, and sends `StartJob` over the daemon-controlled session.
  - Expected initial failure mode: `worker_session` module and session manager API do not exist.

- `attach_worker_valid_hello_start_job_contains_payload_and_layout`
  - Intent: proves the job payload, repo, mode, and layout dirs travel in `StartJob` after authenticated attach, not in process args.
  - Expected initial failure mode: no start-job command construction exists.

- `attach_worker_wrong_token_rejects_without_start_job`
  - Intent: proves token mismatch rejects the stream and leaves no command channel attached.
  - Expected initial failure mode: no token registry/session validation exists.

- `attach_worker_unknown_worker_rejects_without_start_job`
  - Intent: proves spoofed worker ids are rejected.
  - Expected initial failure mode: no session validation API exists.

- `attach_worker_duplicate_hello_rejects_second_attach`
  - Intent: proves a one-time token and active session cannot be reused after a valid attach.
  - Expected initial failure mode: no attached-session tracking exists.

- `attach_worker_after_terminal_status_rejects_without_start_job`
  - Intent: proves late attaches after failed/succeeded/stopped state cannot resurrect terminal workers.
  - Expected initial failure mode: no terminal-status guard in session attach.

- `attach_deadline_without_hello_marks_worker_failed_and_redacts_reason`
  - Intent: proves accepted/starting workers that do not attach before the deadline become failed with a redacted attach-timeout reason.
  - Expected initial failure mode: no attach deadline scheduler exists.

- `attach_deadline_after_successful_attach_does_not_fail_worker`
  - Intent: proves the timeout is cancelled or ignored once the worker attaches successfully.
  - Expected initial failure mode: no deadline cancellation exists.

- `worker_status_update_running_updates_registry_and_appends_event`
  - Intent: proves authenticated worker status frames update daemon-visible state and event history.
  - Expected initial failure mode: no worker-frame ingestion exists.

- `worker_status_update_terminal_succeeded_marks_succeeded_and_closes_session`
  - Intent: proves terminal worker frames map to terminal registry status and close command routing.
  - Expected initial failure mode: no worker-frame terminal handling exists.

- `worker_event_waiting_for_input_marks_waiting_and_records_request`
  - Intent: proves input-request events from the worker create daemon-owned pending input state and stream/list visibility.
  - Expected initial failure mode: no worker event ingestion path exists.

- `send_answer_attached_worker_forwards_answer_over_command_stream`
  - Intent: proves `CommandSender` forwards daemon-validated answers to the attached worker as `DaemonFrame::AnswerInputCommand`.
  - Expected initial failure mode: current command sender is only a fake trait in `service.rs`; no real session-backed sender exists.

- `send_answer_detached_worker_preserves_pending_input_and_returns_unavailable`
  - Intent: proves answering a waiting worker without an active session fails clearly and does not clear pending input.
  - Expected initial failure mode: no session-backed command sender exists.

### Existing tests to keep green

- Keep `packages/daemon/tests/unit/service.rs` dispatch/list/stream/answer tests green, updating only expectations that must change for process/session ownership. In particular, keep dispatch validating prompt/repo/root before side effects, repo identity redaction, daemon-mediated answer validation, and command failure preserving pending input.
- Keep `packages/daemon/tests/unit/registry.rs` event replay, input, terminal, and restart tests green; do not move restart behavior into process/session tests except for child/orphan cleanup.
- Keep `packages/daemon/tests/unit/server.rs` service construction tests green while injecting the real process launcher/session sender through fakeable boundaries.

## Red command plan

Run these immediately after the test writer adds the red tests and minimal include harness entries:

```text
cargo test -p daemon process --no-fail-fast
cargo test -p daemon worker_session --no-fail-fast
```

Expected red state: compile failures for missing `process` and `worker_session` modules/APIs or assertion failures proving missing binary resolution, command construction, authenticated attach, deadline, child exit, and shutdown behavior. If failures are only syntax/import/setup errors, fix the tests before handing to code writer.

Then run the focused daemon suite to confirm the new red tests are the only intentional failures:

```text
cargo test -p daemon --no-fail-fast
```

## Regression command plan

Use the effort regression suites after implementation/refactor:

```text
cargo test -p daemon --no-fail-fast
cargo build -p daemon --bin doric-daemon
cargo clippy -p daemon --all-targets -- -D warnings
npx nx run daemon:build
npx nx run daemon:test
```

Recommended workspace safety checks if process/session changes touch shared lifecycle types or package wiring:

```text
cargo fmt --all -- --check
cargo test -p lifecycle --no-fail-fast
cargo build -p worker --bin doric-worker
```

## Scope and defer notes

- Preserve effort 07 scope: daemon process spawning and daemon-owned worker session management only.
- Do not introduce direct CLI-to-worker communication. All job payload, answer, and shutdown flows must be daemon-mediated over lifecycle gRPC/session frames.
- Treat `packages/worker/src/main.rs` and worker internals as read-only until later efforts. The daemon may know only the worker executable path, process arguments, one-time token, worker id, daemon address, worker root/layout, and lifecycle protocol.
- Defer tests that require real prompt-agent execution, repo clone behavior, provider/runtime composition, tool registration, or live worker runtime attach implementation to efforts 08 through 12.
- Process tests should use fake spawner/child handles and fake clocks/deadlines where feasible; avoid requiring real OS child processes except for a later smoke test if the validator accepts it.
- Session tests should use fake in-memory streams/channels or direct frame handlers rather than binding a real gRPC socket until server/integration efforts need it.
- Do not add protocol fields unless a proposed test cannot be satisfied by the existing `WorkerSessionService`, `WorkerHello`, `StartJob`, `AnswerInputCommand`, and `ShutdownWorker` messages.

## Blocking questions

None. The current lifecycle proto and daemon registry/service seams are sufficient to plan the red tests. The exact public names of new process/session APIs can be chosen by the test writer and code writer, but they should preserve fakeable dependency injection.

## Coordinator decision

accepted
