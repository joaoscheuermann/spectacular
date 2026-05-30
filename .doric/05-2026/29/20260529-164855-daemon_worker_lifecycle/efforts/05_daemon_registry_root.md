# Effort: daemon registry root

Status: todo

## Requirement links

- Features: F-03, F-06, F-09, F-10, F-11
- PRD: FR-4, FR-5, FR-6, FR-9, FR-11, FR-13, FR-14; AC-5, AC-6, AC-9, AC-11, AC-13, AC-14
- TDD: daemon internal state, root/repo handling, in-memory registry tournament, root validation

## Goal

Implement daemon-local root resolution and the in-memory registry/event model without binding a server or spawning workers yet.

## Sequence

- Position: 05 of 15
- Previous effort: 04_cli_lifecycle_parse_routing.md
- Enables: daemon services can be tested against deterministic registry/root behavior before process and gRPC integration.

## Target files

- `packages/daemon/Cargo.toml`
- `packages/daemon/src/lib.rs`
- `packages/daemon/src/root.rs`
- `packages/daemon/src/registry.rs`
- `packages/daemon/src/event.rs`
- `packages/daemon/src/error.rs`
- `packages/daemon/tests/unit/root.rs`
- `packages/daemon/tests/unit/registry.rs`
- `Cargo.lock`

## Coupled files

- `packages/lifecycle/src/identity.rs`, `status.rs`, `event.rs`, and `redaction.rs` are direct dependencies.
- `packages/config/src/lib.rs` supplies the default `config_dir()/workers` root convention.
- `packages/worker` remains read-only and is not spawned in this effort.

## Ownership

- Intended worker write scope: daemon library modules and daemon unit tests listed above.
- Read-only context: lifecycle public types and config path APIs.
- Known conflict risks: registry status/event names become daemon and CLI contracts; coordinate changes with lifecycle type owners.

## Tests to add or update

- Root resolution tests for explicit `--worker-root`, default `config_dir()/workers`, missing/inaccessible root, file-instead-of-directory, and pre-execution failure messages.
- Registry tests for insert/list, status transitions, event sequence allocation, retained replay from sequence `0`, history-truncated marker, pending input validation, duplicate answer rejection, stale answer rejection, terminal records, and unknown/untracked workers.

## Regression suites

- `cargo test -p daemon --no-fail-fast`
- `cargo test -p lifecycle --no-fail-fast`
- `cargo clippy -p daemon --all-targets -- -D warnings`
- `npx nx run daemon:test`

## Acceptance criteria

- The daemon registry is explicitly in-memory.
- List summaries include id, mode, redacted repo identity, status, current activity or terminal reason, and timing or ordering signal.
- Event rings are bounded and support replay-before-live semantics with a history-truncated signal.
- Pending input is keyed by worker id plus request id and rejects unknown, duplicate, stale, or non-waiting answers.
- Invalid worker root fails before job execution and produces a clear root-configuration error.

## Notes

- This effort owns daemon state only. It does not open sockets, spawn `doric-worker`, or implement CLI clients.
- Preserve the repaired decision that stale workers after daemon restart are untracked/unknown rather than reported live.
