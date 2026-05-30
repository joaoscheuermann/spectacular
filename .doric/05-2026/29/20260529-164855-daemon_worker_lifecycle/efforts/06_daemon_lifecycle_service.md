# Effort: daemon lifecycle service

Status: done

## Requirement links

- Features: F-03, F-05, F-09, F-10, F-11
- PRD: FR-3, FR-4, FR-5, FR-7, FR-8, FR-9, FR-13, FR-14; AC-1, AC-2, AC-4, AC-5, AC-6, AC-7, AC-8, AC-9, AC-11, AC-14, AC-15
- TDD: CLI-facing `LifecycleService` gRPC contract, daemon dispatch/list/stream/answer flows, daemon-mediated human input

## Goal

Implement the daemon's CLI-facing lifecycle service over the registry using fakeable worker-launch and stream dependencies.

## Sequence

- Position: 06 of 15
- Previous effort: 05_daemon_registry_root.md
- Enables: CLI client rendering and integration tests can target real daemon service behavior before real process spawning lands.

## Target files

- `packages/daemon/Cargo.toml`
- `packages/daemon/src/lib.rs`
- `packages/daemon/src/service.rs`
- `packages/daemon/src/server.rs`
- `packages/daemon/src/registry.rs`
- `packages/daemon/tests/unit/service.rs`
- `packages/daemon/tests/unit/server.rs`
- `Cargo.lock`

## Coupled files

- `packages/lifecycle/proto/doric/lifecycle/v1.proto` defines generated service types consumed here.
- `packages/daemon/src/process.rs` is a later real spawner consumer.
- `packages/cli/src/main/lifecycle.rs` is a later client consumer.

## Ownership

- Intended worker write scope: daemon service/server modules, daemon service tests, and daemon manifest dependencies.
- Read-only context: lifecycle generated proto, daemon registry/root modules, and TDD service flow.
- Known conflict risks: service request/response mapping is a cross-package API. Keep generated contract changes in lifecycle efforts only unless a blocker is found.

## Tests to add or update

- Dispatch service tests using a fake worker launcher that records accepted jobs without spawning a process.
- List service tests for empty registry, active rows, waiting rows, failed rows, and terminal rows.
- Stream service tests for ordered replay, live tail subscription, history truncation, and unknown worker errors.
- Answer service tests for pending input success, unknown worker, unknown request, duplicate answer, and continuation event.

## Regression suites

- `cargo test -p daemon --no-fail-fast`
- `cargo test -p lifecycle --no-fail-fast`
- `cargo clippy -p daemon --all-targets -- -D warnings`
- `npx nx run daemon:test`

## Acceptance criteria

- Dispatch validates prompt, repo, mode, and root before creating a registry record.
- Dispatch returns stable id, requested mode, redacted repo identity, and initial accepted/starting status.
- List and stream are daemon-mediated and never require direct worker access.
- Answer validates pending input in the daemon before forwarding through a fakeable command channel.
- Feature and debug share service behavior while preserving the requested mode label.

## Notes

- This effort should introduce traits or small dependency seams for worker launch and command delivery so process/session code can land later without rewriting service tests.
- Keep daemon service errors concise and redacted; do not leak worker tokens or raw repo credentials.
