# Effort: lifecycle domain redaction

Status: todo

## Requirement links

- Features: F-02, F-06, F-09, F-10, F-11, F-12
- PRD: FR-3, FR-8, FR-9, FR-12, FR-13, FR-14; AC-1, AC-2, AC-7, AC-9, AC-11, AC-12, AC-14
- TDD: shared lifecycle DTOs, lifecycle redaction helpers, redaction/secrets Dependency Hop

## Goal

Add hand-written lifecycle domain wrappers and central redaction helpers so daemon, worker, and CLI output share one identity and secret-safety layer.

## Sequence

- Position: 03 of 15
- Previous effort: 02_lifecycle_proto_codegen.md
- Enables: daemon registry, worker repo preparation, and CLI rendering can reuse stable ids, request ids, repo identity, status, event, and redaction helpers.

## Target files

- `packages/lifecycle/src/lib.rs`
- `packages/lifecycle/src/identity.rs`
- `packages/lifecycle/src/repo.rs`
- `packages/lifecycle/src/status.rs`
- `packages/lifecycle/src/event.rs`
- `packages/lifecycle/src/redaction.rs`
- `packages/lifecycle/tests/unit/redaction.rs`
- `packages/lifecycle/tests/unit/domain.rs`

## Coupled files

- `packages/daemon/src/registry.rs` and `packages/daemon/src/service.rs` will consume the status/event/id types in later efforts.
- `packages/worker/src/repo.rs` will consume repo redaction and identity helpers in a later effort.
- `packages/cli/src/main/output.rs` will consume redacted display data in a later effort.

## Ownership

- Intended worker write scope: lifecycle hand-written modules and lifecycle unit tests.
- Read-only context: config masking behavior in `packages/config/src/lib.rs` and existing provider error redaction conventions in `packages/llms`.
- Known conflict risks: public lifecycle type names become cross-package contracts; keep names narrow and aligned to PRD/TDD vocabulary before daemon or worker depend on them.

## Tests to add or update

- Add URL credential redaction tests for `https://user:pass@host/path`, token-like usernames, and query/fragment preservation where safe.
- Add API-key-like token redaction tests for lifecycle failure text.
- Add parsing/display tests for `WorkerId` and `RequestId`.
- Add status/event conversion tests where hand-written wrappers bridge generated proto values.

## Regression suites

- `cargo test -p lifecycle --no-fail-fast`
- `cargo clippy -p lifecycle --all-targets -- -D warnings`
- `npx nx run lifecycle:test`

## Acceptance criteria

- Repo credentials are stripped before a repo identity can be stored or rendered.
- Redaction helpers are reusable by daemon, worker, and CLI without depending on those packages.
- Worker ids and request ids have stable display/parse behavior suitable for CLI commands and tests.
- Lifecycle status/event wrappers cover accepted/starting, running, waiting for input, succeeded, failed, stopped, unavailable or untracked, and history-truncated stream events.
- Tests prove redaction without introducing sandboxing or path-confinement claims.

## Notes

- This effort is the graph hub for user-visible secret safety. Later packages should call into it instead of inventing local redaction.
- Preserve scope honesty: redaction is not authorization, sandboxing, or tool confinement.
