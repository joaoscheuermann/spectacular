# Effort 13 Test Planner Receipt

## Role

Doric development test planner for `efforts/13_cli_daemon_client_output.md`.

## Spawn proof

- Required-agent row: `STATE.md` lists development effort `efforts/13_cli_daemon_client_output.md`, role `test planner`, receipt `agents/133_effort_13_test_planner.md`, agent id `019e7768-ce16-70b0-b04c-3b7cf4964cee`, status `spawned`.
- Active effort: `STATE.md` cursor points at `efforts/13_cli_daemon_client_output.md`.
- Previous transition: `agents/131_effort_12_done_transition.md` records effort 12 as accepted and closed.

## Inputs read

- `.agents/skills/coding-conventions/SKILL.md`
- `.agents/skills/coding-conventions/references/implementation-standards.md`
- `.agents/skills/coding-conventions/references/architecture-principles.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/STATE.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/efforts/13_cli_daemon_client_output.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/TDD.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/FEATURES.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/131_effort_12_done_transition.md`
- `packages/cli/src/main/lifecycle.rs`
- `packages/cli/src/main/entry.rs`
- `packages/cli/src/main/output.rs`
- `packages/cli/src/main/cli_types.rs`
- `packages/cli/tests/unit/main_cli.rs`
- `packages/cli/tests/unit/entry.rs`
- `packages/cli/tests/debug_log_startup_test.rs`
- `packages/daemon/src/service.rs`
- `packages/daemon/src/server.rs`
- `packages/lifecycle/src/proto.rs`
- `packages/lifecycle/src/event.rs`
- `packages/lifecycle/src/repo.rs`
- `packages/lifecycle/src/redaction.rs`

## Applicable conventions

- Tests belong under package `tests/`; source may only add minimal `#[cfg(test)] include!(...)` harnesses.
- Tests should be fast, isolated, repeatable, self-validating, and red-first.
- External dependencies must be injected or faked. For this effort, no real daemon process, no network repo, no live socket, no live LLM, and no direct worker fallback.
- The CLI should depend on a small semantic daemon-client seam, not on ad hoc output stubs or daemon internals.

## Current seam findings

- CLI command parsing and entry routing exist in `cli_types.rs` and `entry.rs`.
- Lifecycle handling in `lifecycle.rs` is synchronous and currently returns stub strings through `output.rs`.
- `output.rs` prints `[queued] Lifecycle <command>` plus `Status: daemon client not wired`; that must be replaced by daemon-mediated output.
- `packages/cli/Cargo.toml` does not yet depend on `lifecycle`, `daemon`, or tonic client runtime pieces.
- `daemon::service::LifecycleService` already has plain Rust dispatch/list/stream/answer methods and fakeable launcher/command sender/id seams.
- `daemon::server::build_production_service` exists, but no CLI `doric daemon` production entry seam is wired yet.
- `lifecycle` already owns proto types, status/event/domain DTOs, repo URL redaction, and API-key-like failure redaction.

## Production seams the red tests should force

1. Add a CLI-local daemon-client abstraction in `packages/cli/src/main/lifecycle.rs`, such as `LifecycleClient` or `DaemonClient`, with fake implementations in tests.
   - Required operations: `dispatch`, `list`, `stream_worker`, and `answer_input`.
   - It should accept parsed CLI inputs and return lifecycle/domain/proto DTOs without opening real sockets in tests.
2. Add a daemon-runner dependency seam for `doric daemon`.
   - Entry/routing tests should prove the daemon command calls a run/build function and never falls into lifecycle client or chat.
   - Production can later call daemon library code, but tests should use a fake runner.
3. Convert lifecycle command handling from stub formatting into a result-returning path.
   - Required error type should distinguish invalid input, daemon unavailable/connect failure, unknown worker/request, and daemon/service errors.
   - `entry::AppError` should gain lifecycle error mapping instead of treating lifecycle as a string-only stub.
4. Add output renderers that format response DTOs, not raw command args.
   - Dispatch/list/stream/answer output should be stable after ANSI stripping.
   - Rendering must not print raw repo credentials, provider tokens, environment values, or worker attach tokens.
5. Preserve chat debug-log startup as a bare-chat-only dependency.
   - Lifecycle commands and `doric daemon` must bypass chat debug-log creation.

## Target test files

- `packages/cli/tests/unit/lifecycle_output.rs`
  - New primary unit test file for daemon client fakes and lifecycle output rendering.
  - Include from a minimal source harness, preferably `packages/cli/src/main/lifecycle.rs` if testing the client-facing handler, or `packages/cli/src/main/output.rs` if splitting pure output renderer tests.
- `packages/cli/tests/unit/main_cli.rs`
  - Add or adjust parse/routing tests for lifecycle command grammar and user-visible labels.
- `packages/cli/tests/unit/entry.rs`
  - Add routing tests for daemon command, daemon-unavailable lifecycle errors, lifecycle debug-log bypass, and bare chat preservation.
- `packages/cli/tests/debug_log_startup_test.rs`
  - Add process-level smoke that lifecycle commands preserve stale debug-log content, matching the existing config behavior.
- `packages/cli/src/main/lifecycle.rs`
  - Minimal test harness and client/handler seam forced by tests.
- `packages/cli/src/main/output.rs`
  - Minimal test harness if output renderers stay private here.
- `packages/cli/src/main/entry.rs`
  - Dependency seam expansion for daemon runner and async lifecycle client handling.
- `packages/cli/src/main/cli_types.rs`
  - Only if parse behavior or help wording needs adjustment.
- `packages/cli/Cargo.toml` and `Cargo.lock`
  - Later implementation dependency updates, not part of test planner write ownership.

## Fake dependencies

- `RecordingLifecycleClient`
  - Records calls in order.
  - Configurable responses for dispatch/list/stream/answer.
  - Configurable errors: unavailable, unknown worker, unknown request, invalid request, generic daemon error.
- `RecordingDaemonRunner`
  - Records addr and worker-root passed by `doric daemon`.
  - Can return success or startup error without binding a socket.
- `StaticWorkerEventStream`
  - Provides a finite vector of retained events in exact order.
  - Supports a `history_truncated` event followed by normal worker events.
  - No live follow test unless implemented through an in-memory fake stream without sockets.
- `NoCallLifecycleClient`
  - Panics or records failure if invalid prompt/repo reaches daemon dispatch.
- Existing daemon service fakes are already covered in `packages/daemon/tests/unit/service.rs`; CLI tests should not duplicate daemon registry logic.

## Red-test order

1. `packages/cli/tests/unit/lifecycle_output.rs`
   - `handle_lifecycle_dispatch_feature_success_renders_worker_summary_and_prompt_scope`
   - Arrange fake dispatch response: worker id `feature-123`, mode feature, repo `https://github.com/org/repo.git`, status accepted.
   - Assert output contains id, `Mode: feature`, repo, `Status: accepted`, and prompt/requirements wording.
   - Assert output does not contain `daemon client not wired`, `PRD`, `technical design`, `implementation`, or `handover`.
   - Expected red: file/harness missing or current stub output has no daemon response fields and still says daemon client not wired.

2. `packages/cli/tests/unit/lifecycle_output.rs`
   - `handle_lifecycle_dispatch_debug_success_renders_mode_label_and_shared_v1_wording`
   - Arrange fake dispatch response with mode debug.
   - Assert `Mode: debug` is visible and wording still says prompt/requirements workflow, not a distinct debug workflow.
   - Expected red: current stub preserves command name only and has no v1 scope wording.

3. `packages/cli/tests/unit/lifecycle_output.rs`
   - `handle_lifecycle_dispatch_blank_prompt_rejects_without_client_call`
   - Use blank or whitespace prompt with `NoCallLifecycleClient`.
   - Assert nonzero lifecycle error names `prompt` and no client call occurred.
   - Expected red: clap only requires the flag presence; current handler forwards/prints blank prompt.

4. `packages/cli/tests/unit/lifecycle_output.rs`
   - `handle_lifecycle_dispatch_blank_repo_rejects_without_client_call`
   - Same pattern for whitespace repo.
   - Assert error names `repo` and no client call occurred.
   - Expected red: current handler forwards/prints blank repo.

5. `packages/cli/tests/unit/lifecycle_output.rs`
   - `handle_lifecycle_list_empty_renders_clear_empty_state`
   - Fake list response has no workers.
   - Assert exit-success output says no lifecycle workers are tracked.
   - Expected red: current stub only says lifecycle list and daemon client not wired.

6. `packages/cli/tests/unit/lifecycle_output.rs`
   - `handle_lifecycle_list_rows_renders_mode_repo_status_activity_and_sequence`
   - Fake list response includes accepted feature, waiting debug, failed feature, succeeded debug rows.
   - Assert every row includes worker id, mode label, redacted repo, status, latest sequence/order signal, and activity or reason.
   - Expected red: no list row renderer exists.

7. `packages/cli/tests/unit/lifecycle_output.rs`
   - `handle_lifecycle_worker_stream_replays_events_in_daemon_order`
   - Fake stream events: accepted seq 0, starting seq 1, repo_preparation seq 2, prompt_agent_started seq 3, prompt_artifact_written seq 4, prompt_agent_completed seq 5.
   - Assert output order matches sequence order exactly and includes prompt/requirements terminal success.
   - Expected red: current worker output does not call stream and has no event rendering.

8. `packages/cli/tests/unit/lifecycle_output.rs`
   - `handle_lifecycle_worker_stream_history_truncated_renders_truncation_notice_before_events`
   - Fake stream returns `history_truncated` seq 42 before retained events.
   - Assert truncation notice appears before first retained event and is explicit.
   - Expected red: no stream truncation renderer exists.

9. `packages/cli/tests/unit/lifecycle_output.rs`
   - `handle_lifecycle_worker_stream_waiting_for_input_renders_request_handle`
   - Fake event: `waiting_for_input` with request id `request-1`, prompt `Approve repo access?`.
   - Assert output includes waiting status, request id, prompt text, and the answer command shape `doric answer worker-123 request-1 --text`.
   - Expected red: no input request renderer exists.

10. `packages/cli/tests/unit/lifecycle_output.rs`
    - `handle_lifecycle_answer_success_renders_acceptance_and_continuation`
    - Fake answer response `accepted: true`.
    - Assert output includes worker id, request id, and answer accepted/forwarded wording.
    - Assert output does not echo secret-like answer text by default if the answer contains `sk-test_secret_1234567890`.
    - Expected red: current stub echoes answer text raw.

11. `packages/cli/tests/unit/lifecycle_output.rs`
    - `handle_lifecycle_worker_unknown_worker_returns_clear_error`
    - Fake stream error unknown worker.
    - Assert nonzero error says worker is unknown or untracked and does not hang.
    - Expected red: current handler never surfaces daemon errors.

12. `packages/cli/tests/unit/lifecycle_output.rs`
    - `handle_lifecycle_answer_unknown_request_returns_clear_error`
    - Fake answer error unknown/stale request.
    - Assert nonzero error includes worker id and request id context.
    - Expected red: current handler always returns a stub success string.

13. `packages/cli/tests/unit/lifecycle_output.rs`
    - `handle_lifecycle_commands_daemon_unavailable_return_nonzero_without_direct_worker_fallback`
    - Exercise dispatch, list, worker, and answer through fake unavailable client.
    - Assert each returns daemon-unavailable error and fake direct worker fallback remains uncalled.
    - Expected red: no daemon client error path exists.

14. `packages/cli/tests/unit/lifecycle_output.rs`
    - `handle_lifecycle_output_redacts_repo_credentials_everywhere`
    - Feed dispatch/list/stream failure data containing `https://user:pass@github.com/org/repo.git` and `https://ghp_secret@github.com/org/repo.git`.
    - Assert raw credentials/userinfo are absent and redacted repo identity remains.
    - Expected red: current dispatch stub prints raw repo arg.

15. `packages/cli/tests/unit/lifecycle_output.rs`
    - `handle_lifecycle_output_redacts_provider_and_env_secret_text`
    - Feed daemon/event failure text containing `sk-test_secret_1234567890abcdef`, `OPENAI_API_KEY=sk-test_secret_1234567890abcdef`, and OAuth/token wording.
    - Assert secret values are absent and `[REDACTED]` or equivalent appears.
    - Expected red: no lifecycle error/event redaction path exists in CLI output.

16. `packages/cli/tests/unit/main_cli.rs`
    - `try_parse_from_feature_and_debug_preserve_v1_prompt_only_labels`
    - Keep existing parse tests and add assertions on help/about text if labels are introduced.
    - Assert feature/debug remain separate user-facing modes while sharing prompt/requirements v1 wording.
    - Expected red: parser may pass; wording/output assertions should fail until output model exists.

17. `packages/cli/tests/unit/entry.rs`
    - `dispatch_daemon_command_runs_daemon_runner_without_chat_or_lifecycle_client`
    - Fake dependencies record daemon runner, lifecycle client, config, debug logger, and chat.
    - Assert only daemon runner is called for `Command::Daemon`.
    - Expected red: current entry routes daemon through generic lifecycle stub.

18. `packages/cli/tests/unit/entry.rs`
    - `dispatch_lifecycle_unavailable_returns_error_without_direct_worker_fallback`
    - Fake lifecycle client returns unavailable.
    - Assert `AppError` formats to daemon-unavailable and no chat/debug/config path is touched.
    - Expected red: current lifecycle dependency returns `Option<String>` and cannot represent unavailable.

19. `packages/cli/tests/unit/entry.rs`
    - `dispatch_lifecycle_commands_bypass_chat_debug_log_startup`
    - Expand existing lifecycle debug-log bypass coverage across feature, debug, list, worker, answer, and daemon.
    - Expected red: daemon currently enters lifecycle handler rather than daemon runner; other commands may need async client seam.

20. `packages/cli/tests/unit/entry.rs`
    - `dispatch_bare_invocation_preserves_chat_path`
    - Keep existing test as a regression while changing dependencies.
    - Assert bare `doric` still creates debug logger and then chat.
    - Expected red only if entry seam refactor regresses bare chat.

21. `packages/cli/tests/debug_log_startup_test.rs`
    - `doric_process_lifecycle_commands_preserve_stale_debug_log_content`
    - Run process-level `doric list`, and if daemon-runner can be faked only at unit level, at least a lifecycle command that should fail daemon-unavailable.
    - Assert stale debug log content remains unchanged.
    - Expected red: may initially fail until lifecycle process path bypasses chat debug-log startup and returns daemon-unavailable without touching the log.

## Output contract checklist

- Dispatch success must print:
  - worker id
  - `Mode: feature` or `Mode: debug`
  - redacted repo identity
  - status
  - prompt/requirements-only v1 wording
- List empty must print a successful empty state.
- List rows must print:
  - worker id
  - mode
  - redacted repo
  - status
  - activity or reason
  - latest sequence or order signal
- Stream output must print events in daemon sequence order and surface:
  - history truncation
  - waiting-for-input request id and prompt
  - prompt-agent-only milestones
  - terminal success/failure
- Answer success must print accepted/forwarded state, worker id, and request id.
- All lifecycle failures must be concise, nonzero, and redacted.

## Focused red commands

Run after the test writer adds each slice, expecting targeted failures before implementation:

```text
cargo test -p cli handle_lifecycle_dispatch_feature_success_renders_worker_summary_and_prompt_scope --no-fail-fast
cargo test -p cli handle_lifecycle_dispatch_debug_success_renders_mode_label_and_shared_v1_wording --no-fail-fast
cargo test -p cli handle_lifecycle_dispatch_blank_prompt_rejects_without_client_call --no-fail-fast
cargo test -p cli handle_lifecycle_list_empty_renders_clear_empty_state --no-fail-fast
cargo test -p cli handle_lifecycle_worker_stream_replays_events_in_daemon_order --no-fail-fast
cargo test -p cli handle_lifecycle_output_redacts_repo_credentials_everywhere --no-fail-fast
cargo test -p cli dispatch_daemon_command_runs_daemon_runner_without_chat_or_lifecycle_client --no-fail-fast
cargo test -p cli doric_process_lifecycle_commands_preserve_stale_debug_log_content --no-fail-fast
```

If a red command fails because the new test harness is not included, treat that as an expected first red for the test writer. Once harnesses compile, red should move to missing client seam, missing output behavior, or stub-output assertions.

## Later green and regression commands

Use the effort-file gates after implementation:

```text
cargo test -p cli --no-fail-fast
cargo test -p lifecycle --no-fail-fast
cargo test -p daemon --no-fail-fast
cargo build -p cli --bin doric
cargo clippy -p cli --all-targets -- -D warnings
npx nx run cli:test
```

Additional narrow commands worth running when the client dependency is introduced:

```text
cargo test -p cli lifecycle --no-fail-fast
cargo test -p cli entry --no-fail-fast
cargo test -p cli debug_log_startup_test --no-fail-fast
```

## Risks

- The CLI may be tempted to call `daemon::service::LifecycleService` directly for normal lifecycle commands. Tests should require a client seam and daemon-unavailable behavior so production does not bypass the daemon authority.
- The fake client seam can become too shallow if it only returns rendered strings. Tests should fake DTO/client responses and assert renderer output separately enough to preserve a meaningful boundary.
- Answer output can leak user-provided secret text if the current stub echo pattern remains. Tests should require non-echo or redacted echo.
- Debug-log startup regression is likely during entry refactor. Keep both unit and process-level debug-log tests.
- `doric daemon` may need daemon package dependencies in `cli`; test writer should keep the first red focused on routing shape, not on socket binding.
- ANSI styling can make brittle assertions. Tests should strip ANSI and assert stable text fragments/order.

## Coordinator handoff

The plan is ready for the effort 13 test writer. It is red-first, offline, fake-driven, and scoped to CLI lifecycle client/output seams while preserving daemon/lifecycle contracts and bare chat behavior.

Coordinator decision: accepted
