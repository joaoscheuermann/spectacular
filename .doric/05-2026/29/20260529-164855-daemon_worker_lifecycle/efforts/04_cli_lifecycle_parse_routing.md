# Effort: CLI lifecycle parse routing

Status: todo

## Requirement links

- Features: F-01, F-05, F-10, F-11
- PRD: FR-1, FR-2, FR-10, FR-12; AC-3, AC-4, AC-10, AC-12, AC-15
- TDD: CLI command surface, CLI entrypoint rewrite, lifecycle commands bypass chat debug-log startup

## Goal

Add the raw lifecycle command grammar and refactor `cli` entrypoint routing so lifecycle and config commands dispatch before chat/provider debug-log startup.

## Sequence

- Position: 04 of 15
- Previous effort: 03_lifecycle_domain_redaction.md
- Enables: later CLI client work can focus on daemon calls and output formatting without risking chat startup regressions.

## Target files

- `packages/cli/src/main/cli_types.rs`
- `packages/cli/src/main/entry.rs`
- `packages/cli/src/main/output.rs`
- `packages/cli/src/main/lifecycle.rs`
- `packages/cli/src/main.rs`
- `packages/cli/tests/unit/main_cli.rs`
- `packages/cli/tests/unit/entry.rs`
- `packages/cli/tests/debug_log_startup_test.rs`

## Coupled files

- `packages/cli/src/chat/mod.rs`, `packages/cli/src/chat/provider.rs`, and `packages/cli/src/chat/runtime_selection.rs` are read-only chat behavior context.
- `packages/cli/src/main/config_ops.rs` is read-only config routing context.
- `packages/daemon` and `packages/lifecycle` are later runtime dependencies for real daemon calls.

## Ownership

- Intended worker write scope: CLI main command parsing/routing/output files and CLI tests listed above.
- Read-only context: chat module and config command behavior.
- Known conflict risks: `entry.rs` is a high-conflict composition root. Keep the edit focused on routing order and explicit lifecycle command stubs/seams.

## Tests to add or update

- Parse tests for `daemon`, `feature`, `debug`, `list`, `worker`, and `answer`.
- Missing `--prompt`, `--repo`, and `--text` cases remain clap errors and do not create lifecycle jobs.
- Entry tests prove lifecycle and config command dispatch do not create `LlmDebugLogger`.
- Existing bare invocation still creates chat debug logging immediately before `chat::run`.
- Update or replace the existing debug-log startup test so config commands no longer require debug-log creation.

## Regression suites

- `cargo test -p cli --no-fail-fast`
- `cargo build -p cli --bin doric`
- `cargo clippy -p cli --all-targets -- -D warnings`
- `npx nx run cli:test`

## Acceptance criteria

- Top-level help lists raw lifecycle commands and still does not expose a `chat` subcommand.
- `doric feature --prompt <p> --repo <r>` and `doric debug --prompt <p> --repo <r>` parse into mode-preserving lifecycle command values.
- `doric answer <worker-id> <request-id> --text <answer>` parses into a daemon-mediated answer command shape.
- Config and lifecycle commands bypass chat/provider debug-log startup.
- Bare `doric` chat behavior is preserved and remains the only branch that creates chat provider debug logging.

## Notes

- This effort may use a temporary lifecycle handler seam if the daemon client is not wired yet. Final daemon behavior belongs to `13_cli_daemon_client_output.md`.
- Preserve the repaired TDD decision that lifecycle commands do not fail because chat debug-log startup failed.
