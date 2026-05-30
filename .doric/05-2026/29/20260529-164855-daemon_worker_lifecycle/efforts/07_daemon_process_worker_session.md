# Effort: daemon process worker session

Status: done

## Requirement links

- Features: F-03, F-04, F-09, F-10, F-11
- PRD: FR-4, FR-6, FR-7, FR-8, FR-9, FR-13, FR-14; AC-4, AC-7, AC-8, AC-9, AC-11, AC-14
- TDD: worker-initiated bidirectional gRPC stream, daemon process spawner, worker attach deadline, restart semantics

## Goal

Wire the daemon's real process-spawn and worker-session management while preserving the daemon as the sole lifecycle authority.

## Sequence

- Position: 07 of 15
- Previous effort: 06_daemon_lifecycle_service.md
- Enables: worker runtime can attach to the daemon and receive `StartJob` over the selected bidirectional gRPC stream.

## Target files

- `packages/daemon/Cargo.toml`
- `packages/daemon/src/lib.rs`
- `packages/daemon/src/process.rs`
- `packages/daemon/src/worker_session.rs`
- `packages/daemon/src/service.rs`
- `packages/daemon/src/server.rs`
- `packages/daemon/src/main.rs`
- `packages/daemon/tests/unit/process.rs`
- `packages/daemon/tests/unit/worker_session.rs`
- `Cargo.lock`

## Coupled files

- `packages/worker/src/main.rs` is read-only until the worker runtime effort implements attach behavior.
- `packages/lifecycle` worker-session service contract is a direct dependency.
- `packages/cli/src/main/lifecycle.rs` will later start `daemon::run` for `doric daemon`.

## Ownership

- Intended worker write scope: daemon process/session/server modules, daemon binary entrypoint, daemon tests, and daemon manifest updates.
- Read-only context: worker scaffold, lifecycle generated service types, and existing CLI entrypoint.
- Known conflict risks: process spawning touches OS-specific behavior. Keep child process resolution and command construction isolated behind a fakeable spawner trait.

## Tests to add or update

- Worker binary resolution tests for configured path/current executable sibling lookup where applicable.
- Spawn command tests prove worker id, daemon address, token, and worker root are passed without raw prompt/repo payload.
- Attach tests for correct token, wrong token, duplicate attach, unknown worker, attach after terminal status, and attach deadline.
- Child exit mapping tests for succeeded, failed, stopped/unavailable, and no-terminal-frame cases.
- Shutdown tests for Ctrl-C or graceful daemon shutdown best-effort worker termination where feasible with fakes.

## Regression suites

- `cargo test -p daemon --no-fail-fast`
- `cargo build -p daemon --bin doric-daemon`
- `cargo clippy -p daemon --all-targets -- -D warnings`
- `npx nx run daemon:build`
- `npx nx run daemon:test`

## Acceptance criteria

- The daemon spawns `doric-worker` as a child process and sends job payload only over daemon-controlled gRPC after authenticated worker attach.
- Worker attach uses a one-time token that is never printed.
- Attach timeout transitions accepted/starting workers to failed with a redacted reason.
- Child exit monitoring updates daemon-visible status and emits terminal events.
- On daemon restart, previous workers are untracked and not reported as live.

## Notes

- Do not add direct CLI-to-worker communication.
- Do not make daemon depend on worker internals beyond process arguments and lifecycle protocol.
