# Effort: CLI daemon client output

Status: done

## Requirement links

- Features: F-01, F-05, F-09, F-10, F-11
- PRD: FR-1 through FR-14 where applicable, especially FR-1, FR-2, FR-3, FR-4, FR-5, FR-7, FR-9, FR-10, FR-12, FR-14; AC-1, AC-2, AC-3, AC-4, AC-5, AC-6, AC-7, AC-8, AC-10, AC-11, AC-12, AC-14, AC-15
- TDD: CLI command surface, CLI output contracts, daemon-mediated dispatch/list/stream/answer, `doric daemon`

## Goal

Wire CLI lifecycle commands to the daemon over gRPC and render stable, redacted, scope-honest command output.

## Sequence

- Position: 13 of 15
- Previous effort: 12_worker_session_runtime.md
- Enables: users can exercise the lifecycle path through raw CLI commands while preserving bare chat behavior.

## Target files

- `packages/cli/Cargo.toml`
- `packages/cli/src/main/lifecycle.rs`
- `packages/cli/src/main/lifecycle_output.rs`
- `packages/cli/src/main/entry.rs`
- `packages/cli/src/main/output.rs`
- `packages/cli/src/main/cli_types.rs`
- `packages/cli/tests/unit/lifecycle_output.rs`
- `packages/cli/tests/unit/main_cli.rs`
- `packages/cli/tests/unit/entry.rs`
- `packages/cli/tests/debug_log_startup_test.rs`
- `Cargo.lock`

## Coupled files

- `packages/lifecycle` generated client and domain/redaction helpers are direct dependencies.
- `packages/daemon/src/server.rs` and `src/service.rs` define daemon service behavior.
- `packages/cli/src/chat/*` remains read-only except for routing tests proving chat still works through the bare invocation path.

## Ownership

- Intended worker write scope: CLI lifecycle client/rendering/routing files, CLI tests, CLI manifest updates, and lockfile dependency changes.
- Read-only context: daemon service implementation, lifecycle contract, chat provider/runtime modules.
- Known conflict risks: `entry.rs` and `output.rs` are shared by config and chat error paths. Keep lifecycle formatting additive and avoid rewriting config/chat output.

## Tests to add or update

- CLI lifecycle output tests for feature dispatch, debug dispatch, list empty, list rows, stream replay, stream history-truncated, input wait, answer success, unknown worker, daemon unavailable, repo credential redaction, and v1 prompt-only wording.
- Entry tests for `doric daemon` calling daemon run path and lifecycle commands returning daemon-unavailable without direct worker fallback.
- Parse/routing tests preserving `feature` and `debug` mode labels while sharing v1 behavior.
- Debug-log startup tests confirming lifecycle commands bypass chat debug-log startup.

## Regression suites

- `cargo test -p cli --no-fail-fast`
- `cargo test -p lifecycle --no-fail-fast`
- `cargo test -p daemon --no-fail-fast`
- `cargo build -p cli --bin doric`
- `cargo clippy -p cli --all-targets -- -D warnings`
- `npx nx run cli:test`

## Acceptance criteria

- Dispatch commands require non-empty prompt and repo, return id/mode/repo/status on success, and create no job on invalid input.
- `list` prints a clear empty state and structured rows for known workers.
- `worker <id>` streams daemon-provided events in order and fails clearly for unknown ids.
- `answer <worker-id> <request-id> --text <answer>` is daemon-mediated and fails clearly for unknown workers or requests.
- Daemon-unavailable errors are nonzero and do not attempt direct worker communication.
- Output redacts repo credentials and does not print environment/provider secrets.
- Successful v1 output identifies prompt/requirements workflow only and preserves feature/debug mode labels without claiming separate internal behavior.

## Notes

- Preserve repaired TDD decisions: lifecycle commands bypass chat debug-log startup, and feature/debug share one prompt/requirements workflow in v1.
- Do not retire bare chat in this effort.
