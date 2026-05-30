# Effort 13 Test Writer Retry Receipt

## Spawn proof

- Role: development test writer retry for `efforts/13_cli_daemon_client_output.md`
- Agent type: worker
- Agent id: 019e776f-e019-7b23-8633-622a7c03a18b
- Spawn result: spawned retry worker recorded in `STATE.md` required-agent row
- Required agents row: `development | efforts/13_cli_daemon_client_output.md | test writer retry | worker | agents/135_effort_13_test_writer_retry.md | 019e776f-e019-7b23-8633-622a7c03a18b | spawned`

## Input artifacts

- `.agents/skills/coding-conventions/SKILL.md`
- `.agents/skills/coding-conventions/references/implementation-standards.md`
- `.agents/skills/coding-conventions/references/sexy-rust.md`
- `.agents/skills/doric/SKILL.md`
- `.agents/skills/doric/references/05-development.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/STATE.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/efforts/13_cli_daemon_client_output.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/133_effort_13_test_planner.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/134_effort_13_test_writer.md`

## Files changed

- `packages/cli/src/main/lifecycle.rs`
- `packages/cli/src/main/entry.rs`
- `packages/cli/tests/unit/lifecycle_output.rs`
- `packages/cli/tests/unit/entry.rs`
- `packages/cli/tests/unit/main_cli.rs`
- `packages/cli/tests/debug_log_startup_test.rs`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/135_effort_13_test_writer_retry.md`

## Tests added or changed

- Replaced `packages/cli/tests/unit/lifecycle_output.rs` with fake-driven lifecycle tests that call `handle_lifecycle_command_with_client(command, client)` and assert the recording client receives exact dispatch/list/stream/answer requests.
- `handle_lifecycle_dispatch_feature_success_renders_worker_summary_and_prompt_scope` forces injected dispatch, feature mode, daemon worker id, repo/status output, prompt/requirements v1 wording, and no stub or later-phase claims.
- `handle_lifecycle_dispatch_debug_success_renders_mode_label_and_shared_v1_wording` forces injected dispatch while preserving the debug label and shared v1 prompt/requirements wording.
- `handle_lifecycle_dispatch_blank_prompt_rejects_without_client_call` and `handle_lifecycle_dispatch_blank_repo_rejects_without_client_call` force local validation before any daemon-client call.
- `handle_lifecycle_list_empty_renders_clear_empty_state` and `handle_lifecycle_list_rows_renders_mode_repo_status_activity_and_sequence` force injected list responses, empty state output, and row fields.
- `handle_lifecycle_worker_stream_replays_events_in_daemon_order`, `handle_lifecycle_worker_stream_history_truncated_renders_truncation_notice_before_events`, and `handle_lifecycle_worker_stream_waiting_for_input_renders_request_handle` force injected stream replay, truncation, and input request output.
- `handle_lifecycle_answer_success_renders_acceptance_and_continuation`, `handle_lifecycle_worker_unknown_worker_returns_clear_error`, and `handle_lifecycle_answer_unknown_request_returns_clear_error` force injected answer/stream success and daemon error handling.
- `handle_lifecycle_commands_daemon_unavailable_return_nonzero_without_direct_worker_fallback` forces every lifecycle command to surface daemon-unavailable from the injected client with no direct worker fallback.
- `handle_lifecycle_output_redacts_repo_credentials_everywhere` and `handle_lifecycle_output_redacts_provider_and_env_secret_text` force repo and provider secret redaction on daemon-provided output/errors.
- `dispatch_daemon_command_runs_daemon_runner_without_chat_or_lifecycle_client` forces a separate daemon-runner dependency in `entry.rs` for `doric daemon`.
- `dispatch_lifecycle_unavailable_returns_error_without_direct_worker_fallback` and `dispatch_lifecycle_commands_bypass_chat_debug_log_startup` keep lifecycle routing off chat/config/debug-log startup.
- `doric_process_lifecycle_commands_preserve_stale_debug_log_content` keeps the production process path red by expecting daemon-unavailable for `doric list` while preserving stale debug-log content.
- Removed direct production `handle_lifecycle_command` output assertions from `main_cli.rs`; parse/help tests remain routing-focused.

## Minimal seam declarations

- Added CLI-local lifecycle daemon-client DTOs and `LifecycleDaemonClient` in `packages/cli/src/main/lifecycle.rs`.
- Added `handle_lifecycle_command_with_client(command, client)` as the precise test-facing API. It currently delegates to the old stub path, so the tests are intentionally red until the code writer wires the injected client.
- Added `run_daemon` to `DispatchDependencies` in `packages/cli/src/main/entry.rs` so daemon command routing is separate from the lifecycle client dependency.

## Red evidence

- Command: `cargo test -p cli handle_lifecycle_dispatch_feature_success_renders_worker_summary_and_prompt_scope --no-fail-fast`
  - Observed result: assertion-red. The recording fake client was not called; expected one `Dispatch` call but observed `[]`.
- Command: `cargo test -p cli handle_lifecycle_dispatch_blank_prompt_rejects_without_client_call --no-fail-fast`
  - Observed result: assertion-red. The injected handler returned old stub success output for a blank prompt instead of returning a validation error before the client call.
- Command: `cargo test -p cli handle_lifecycle_commands_daemon_unavailable_return_nonzero_without_direct_worker_fallback --no-fail-fast`
  - Observed result: assertion-red. The injected handler returned old stub success output instead of propagating the fake client's daemon-unavailable error.
- Command: `cargo test -p cli dispatch_daemon_command_runs_daemon_runner_without_chat_or_lifecycle_client --no-fail-fast`
  - Observed result: passed. This confirms the new entry dependency seam routes `Command::Daemon` to the daemon runner, not lifecycle/chat/config/debug-log paths.
- Command: `cargo test -p cli doric_process_lifecycle_commands_preserve_stale_debug_log_content --no-fail-fast`
  - Observed result: assertion-red. Process `doric list` exits success and prints `[queued] Lifecycle list` / `Status: daemon client not wired`; the test expects daemon-unavailable failure while preserving the stale debug log.
- Command: `cargo fmt -p cli -- --check`
  - Observed result: passed.

## Blocking questions

- None.

## Coordinator decision

Coordinator decision: accepted
