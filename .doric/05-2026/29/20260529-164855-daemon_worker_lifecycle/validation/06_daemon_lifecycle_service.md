# Validation: daemon lifecycle service

## Red evidence

| Command | Exit code | Summary |
| ------- | --------- | ------- |
| `cargo test -p daemon --no-fail-fast service::` | 1 | Expected compile failure because `daemon::service` and `daemon::server` are exported for the effort 06 API but production modules do not exist yet. |

Representative errors:

```text
error[E0583]: file not found for module `server`
error[E0583]: file not found for module `service`
```

Failure mapping:

- Missing `daemon::service` covers dispatch/list/stream/answer service behavior and fakeable launch/command seams.
- Missing `daemon::server` covers loopback bind configuration and service construction without binding.

## Green evidence

| Command | Exit code | Summary |
| ------- | --------- | ------- |
| `cargo fmt --all -- --check` | 0 | Workspace formatting check passed. |
| `cargo test -p daemon --no-fail-fast service::` | 0 | 11 focused service tests passed, including dispatch validation ordering, daemon-mediated list/stream mapping, answer validation before forwarding, and command-failure pending-input preservation. |
| `cargo test -p daemon --no-fail-fast server::` | 0 | 5 focused server tests passed; server config/build paths do not bind a listener. |
| `cargo test -p daemon --no-fail-fast` | 0 | 39 daemon tests passed across root, registry, service, and server modules. |
| `cargo test -p lifecycle --no-fail-fast` | 0 | 18 lifecycle tests passed; no lifecycle proto changes were needed. |
| `cargo clippy -p daemon --all-targets -- -D warnings` | 0 | Daemon clippy passed with warnings denied. |
| `npx nx run daemon:test` | 0 | Nx daemon test target passed; 39 daemon tests passed through `cargo test --target-dir dist/target/daemon -p daemon`. |

Validator note: after a narrow refactor, the first focused service rerun failed once because the new failure-path test compared Rust enum discriminants to proto status values. The assertion was corrected to compare `WorkerStatus::WaitingForInput`, then the required focused and regression gates above passed.

## Focused commands

- `cargo test -p daemon --no-fail-fast service::`
- `cargo test -p daemon --no-fail-fast server::`
- `cargo test -p daemon --no-fail-fast`

## Regression commands

- `cargo test -p lifecycle --no-fail-fast`
- `cargo clippy -p daemon --all-targets -- -D warnings`
- `npx nx run daemon:test`
- `cargo fmt --all -- --check`

## Unavailable tooling

None. All requested validation commands ran.

## Refactors applied

- Split registry answer validation into `Registry::validate_answer(&InputAnswer)` and kept `Registry::answer_input` as the commit path.
- Changed `LifecycleService::answer_input` to validate pending input, forward the command through the injected sender, and only then clear pending input / mark the worker running / append the continuation event. If command forwarding fails, pending input and waiting status remain intact.
- Added `answer_input_command_failure_preserves_pending_input_without_running_status` to lock the command-failure behavior.
- Did not change daemon error taxonomy because `packages/daemon/src/error.rs` is read-only for this validator; ordinary invalid prompt/mode/repo errors still use the existing root-configuration variant and should be revisited by an owner of that file.

## Reviewer decision

green

## Live-tail repair evidence

| Command | Exit code | Summary |
| ------- | --------- | ------- |
| `cargo test -p daemon --no-fail-fast service::` | 1 | Initial repair compile failed because `WorkerEventStream` did not implement `Debug`, which `unwrap_err` requires for the existing unknown-worker stream test. |
| `cargo test -p daemon --no-fail-fast service::` | 0 | 12 focused service tests passed, including `stream_known_worker_replays_then_tails_live_events` for replay followed by an event appended after stream creation. |
| `cargo fmt --all -- --check` | 1 | Formatting check found the new `crate::registry` import needed rustfmt wrapping. |
| `cargo fmt --all` | 0 | Applied rustfmt to the workspace. |
| `cargo test -p daemon --no-fail-fast service::` | 0 | 12 focused service tests passed after formatting. |
| `cargo fmt --all -- --check` | 0 | Workspace formatting check passed after rustfmt. |
| `cargo test -p daemon --no-fail-fast server::` | 0 | 5 focused server tests passed; server config/build paths still do not bind a listener. |
| `cargo test -p daemon --no-fail-fast` | 0 | 40 daemon tests passed across root, registry, service, and server modules. |
| `cargo clippy -p daemon --all-targets -- -D warnings` | 0 | Daemon clippy passed with warnings denied. |

Repair notes:

- Added `Registry::subscribe(worker_id, from_sequence) -> EventSubscription` with retained replay and a synchronous per-worker live receiver.
- `Registry::append_event` now publishes appended events to active subscribers while preserving retained replay/truncation behavior.
- `LifecycleService::stream` now returns `WorkerEventStream`, a daemon-local wrapper exposing `replay()` and `try_next()` for live events appended after stream creation.
- Added `stream_known_worker_replays_then_tails_live_events` to prove replay first, then live-tail delivery without polling private registry internals.
- Existing answer text non-leak behavior remains unchanged because `InputAnswer` still stores only worker/request ids, and command-failure pending-input preservation remains covered by `answer_input_command_failure_preserves_pending_input_without_running_status`.

## Repair reviewer decision

green

## Post-repair validation evidence

| Command | Exit code | Summary |
| ------- | --------- | ------- |
| `cargo fmt --all -- --check` | 0 | Workspace formatting check passed. |
| `cargo test -p daemon --no-fail-fast service::` | 0 | 12 focused service tests passed, including replay-followed-by-live-tail and command-failure pending-input preservation. |
| `cargo test -p daemon --no-fail-fast server::` | 0 | 5 focused server tests passed; service construction remains listener-free. |
| `cargo test -p daemon --no-fail-fast` | 0 | 40 daemon tests passed across root, registry, service, server, binary, and doctest targets. |
| `cargo test -p lifecycle --no-fail-fast` | 0 | 18 lifecycle tests passed; proto/service contract and redaction coverage remain green. |
| `cargo clippy -p daemon --all-targets -- -D warnings` | 0 | Daemon clippy passed with warnings denied. |
| `npx nx run daemon:test` | 0 | Nx daemon test target passed; 40 daemon tests passed through `cargo test --target-dir dist/target/daemon -p daemon`. |

Post-repair notes:

- `Registry::subscribe(worker_id, from_sequence) -> EventSubscription` is the public registry seam for replay plus live delivery.
- `Registry::append_event` retains events and publishes cloned events to active subscribers without changing the retained replay ring.
- `LifecycleService::stream` returns `WorkerEventStream` with `replay()` for retained items and `try_next()` for live events appended after stream creation.
- `stream_known_worker_replays_retained_events_and_truncation` remains green, so live subscribers did not break replay suffix or truncation behavior.
- `stream_known_worker_replays_then_tails_live_events` proves replay first, then live delivery of a `prompt_agent_started` event appended after subscription.
- No real sockets, tonic server start, process spawn, CLI edits, worker edits, or proto edits were introduced by this validation pass.
- Answer text non-leak remains covered by the registry/service design: `InputAnswer` stores only worker/request ids, while answer text is forwarded only through the injected command seam.
- Command-failure pending-input preservation remains covered by `answer_input_command_failure_preserves_pending_input_without_running_status`.

## Post-repair validator decision

green
