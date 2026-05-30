# Validation: daemon registry root

## Red evidence

| Command | Exit code | Summary |
| ------- | --------- | ------- |
| `cargo test -p daemon --no-fail-fast root::` | 1 | Expected compile failure because effort 05 daemon root, event, and registry APIs did not exist yet and `daemon` did not yet depend on `lifecycle`. |

Representative errors:

```text
error[E0432]: unresolved import `crate::root`
error[E0432]: unresolved import `crate::event`
error[E0432]: unresolved import `crate::registry`
error[E0433]: failed to resolve: use of unresolved module or unlinked crate `lifecycle`
```

Failure mapping:

- Missing `crate::root` covered worker-root defaulting, validation, and pre-execution root-configuration failures.
- Missing `crate::registry` covered in-memory list summaries, status transitions, pending input validation, and restart-loss semantics.
- Missing `crate::event` covered event sequence allocation, retained replay, and history-truncated signals.
- Missing `lifecycle` dependency covered the accepted daemon API dependency on `WorkerId`, `RequestId`, `RepoIdentity`, `WorkerStatus`, and `StreamEvent`.

## Green evidence

| Command | Exit code | Summary |
| ------- | --------- | ------- |
| `cargo fmt --all -- --check` | 0 | Workspace formatting check passed before focused/regression validation. |
| `cargo test -p daemon --no-fail-fast root::` | 0 | Root-focused daemon tests passed: 6 passed, 0 failed; binary target had 0 tests. |
| `cargo test -p daemon --no-fail-fast registry::` | 0 | Registry-focused daemon tests passed: 17 passed, 0 failed; binary target had 0 tests. |
| `cargo test -p daemon --no-fail-fast` | 0 | Full daemon package tests passed: 23 passed, 0 failed; binary/doc targets had 0 tests. |
| `cargo test -p lifecycle --no-fail-fast` | 0 | Lifecycle regression tests passed: 18 passed, 0 failed; doc tests had 0 tests. |
| `cargo clippy -p daemon --all-targets -- -D warnings` | 0 | Daemon clippy passed with warnings denied. |
| `npx nx run daemon:test` | 0 | Nx daemon test target passed: 23 daemon tests passed, 0 failed. Local npm emitted an experimental CommonJS/ESM warning outside the daemon target. |

## Focused commands

- `cargo test -p daemon --no-fail-fast root::` - green.
- `cargo test -p daemon --no-fail-fast registry::` - green.
- `cargo test -p daemon --no-fail-fast` - green.

## Regression commands

- `cargo test -p lifecycle --no-fail-fast` - green.
- `cargo clippy -p daemon --all-targets -- -D warnings` - green.
- `npx nx run daemon:test` - green.
- `cargo fmt --all -- --check` - green.

## Unavailable tooling

None. All requested validation commands ran.

## Refactors applied

- Added a practical worker-root writable check in `validate_worker_root` by creating and removing a temporary probe directory under the canonical root. Missing roots and file roots still fail before worker layout or execution.
- Removed raw answer text from daemon answer continuation events. `InputAnswer::new` still accepts the answer text for the future command-forwarding seam, but the in-memory registry records only `answer_provided` with the concise message `input answered; continuing`.
- Removed the unused `temp` fixture variable from the root default-provider error test.
- Added a focused assertion that answer replay events do not expose the submitted answer body.

## Reviewer decision

Accepted. Effort 05 remains explicitly in-memory, uses lifecycle domain types, validates roots before worker layout, does not bind a server, and does not spawn workers.
