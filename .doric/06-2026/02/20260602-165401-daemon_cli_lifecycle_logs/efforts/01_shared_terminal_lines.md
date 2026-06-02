# Effort: Shared lifecycle terminal lines

Status: done

## Requirement links

- FEATURES.md: F1, F8.
- PRD.md: FR3, FR4, FR12, FR13, FR21, FR23, FR24; User Value Hops 7, 8, 9.
- TDD.md: Proposed design component 1; Dependency Hops 1 and 10; testing strategy lifecycle formatter and safe-line cases.

## Goal

Add shared plain-text lifecycle terminal primitives in the `lifecycle` package so daemon and CLI callers can render `[<timestamp>] <message>` lines with one-line, redacted, terminal-safe message text.

## Sequence

- Position: 01 of 07.
- Previous effort: none.
- Next effort: 02_repo_url_validation.md.
- Dependency reason: downstream repo validation, daemon logging, CLI rendering, and worker clone wording all need the shared safe terminal rendering contract before they can consume it consistently.

## Target files

- `packages/lifecycle/src/terminal.rs` (new): own timestamp formatting, safe-message rendering, and `format_line` behavior.
- `packages/lifecycle/src/redaction.rs`: extend reusable secret redaction only where terminal-safe rendering needs existing redaction helpers.
- `packages/lifecycle/src/lib.rs`: export the new terminal module.
- `packages/lifecycle/tests/unit/redaction.rs` and/or `packages/lifecycle/tests/unit/domain.rs`: add behavior tests for safe one-line output and timestamped line formatting.
- `packages/lifecycle/Cargo.toml`: add a direct timestamp dependency only if the implementation uses one in `lifecycle`.
- `Cargo.lock`: update only if a dependency is added.

Graph impact: `lifecycle::terminal` sits below `cli`, `daemon`, and `worker`. It must not depend on those packages. `redaction.rs` remains a shared helper node, and callers should consume terminal-safe display strings rather than duplicating CR/LF/ESC filtering.

## Coupled files

- `packages/cli/src/main/lifecycle_output.rs`: later consumes `format_line` and safe-message helpers.
- `packages/daemon/src/service.rs` and `packages/daemon/src/worker_session.rs`: later daemon logger calls must use the same terminal contract.
- `packages/worker/src/event.rs` and `packages/worker/src/runtime.rs`: later clone-start and worker event display text must remain compatible with safe rendering.
- `docs/architecture-and-packages.md`: read-only unless later implementation changes package responsibilities.

Graph impact: callers are coupled through shared display semantics only. No CLI styling, daemon process behavior, worker runtime state, or JSON/structured logging should move into `lifecycle`.

## Ownership

- Intended worker write scope: `packages/lifecycle/src/terminal.rs`, `packages/lifecycle/src/redaction.rs`, `packages/lifecycle/src/lib.rs`, lifecycle unit tests, and lifecycle manifest/lockfile only if a direct timestamp dependency is required.
- Read-only context: PRD/TDD/FEATURES artifacts, existing lifecycle event/status/repo modules, CLI output, daemon service/session, worker runtime/event.
- Known conflict risks: existing uncommitted edits in lifecycle proto/status files are outside this effort unless implementation proves they are required. Preserve them and do not revert them.

## Tests to add or update

- Add lifecycle unit tests proving `format_line` returns `[<timestamp>] <safe-message>` for a fixed timestamp.
- Add tests proving CR, LF, ESC, and other terminal control characters cannot create additional displayed lifecycle lines.
- Add tests proving API-key-like and credential-bearing text is redacted before display.
- Add tests proving ordinary safe text remains readable and concise.

## Regression suites

- `cargo test -p lifecycle`

Graph impact: this suite validates the shared root node before daemon, CLI, and worker efforts depend on it. No daemon, CLI, or worker suites are required in this first slice unless the implementation leaks package coupling.

## Acceptance criteria

- Shared lifecycle line formatting exists in `lifecycle` and returns plain text, not styled or structured output.
- Timestamp rendering uses the repository's existing style where practical or a compact RFC3339/ISO-style fallback.
- The direct timestamp dependency decision is explicit in code/tests: if `lifecycle` owns timestamp formatting, the dependency belongs in `packages/lifecycle/Cargo.toml`; if no new dependency is needed, tests still prove the selected format.
- Safe-message rendering redacts known secrets and normalizes line breaks/control characters into one displayed line.
- No JSON logs, rich TUI output, progress spinner behavior, nested structured logs, or CLI styling is introduced.
- Existing lifecycle domain tests continue to pass.

## Notes

- Keep the module deep and small: timestamp-plus-message formatting and safe display helpers only.
- This effort should not implement repository URL validation; that is the next isolated slice.
