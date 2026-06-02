# Effort: CLI stream lifecycle output

Status: todo

## Requirement links

- FEATURES.md: F1, F7, F8.
- PRD.md: FR3, FR8, FR9, FR10, FR11, FR12, FR13, FR20, FR21, FR23; User Value Hops 6, 7, 8, and 9.
- TDD.md: Proposed design component 6; Dependency Hops 7, 10, and 12; CLI worker stream contract; testing strategy stream and fast-event cases.

## Goal

Render CLI worker event streams as one timestamped safe lifecycle line per stream item, beginning with `started`, using event timestamps when present and render-time fallback when absent.

## Sequence

- Position: 06 of 07.
- Previous effort: 05_cli_feature_list_output.md.
- Next effort: 07_worker_clone_wording_regressions.md.
- Dependency reason: stream rendering depends on shared lifecycle formatter, daemon occurred-at timestamps, and the CLI connection seam established by previous efforts. Worker clone wording lands after this renderer can display it.

## Target files

- `packages/cli/src/main/lifecycle.rs`: replace the old stream header/no-retained-events flow with `[<timestamp>] started` and timestamped item writes.
- `packages/cli/src/main/lifecycle_client.rs`: carry `WorkerEvent.occurred_at` into CLI stream item models while preserving fallback behavior for missing timestamps.
- `packages/cli/src/main/lifecycle_output.rs`: render history truncation, worker events, waiting-for-input, failures, completion, and clone activity as one safe timestamped line each.
- `packages/cli/tests/unit/lifecycle_output.rs`: update stream tests for started first, clone wording display, timestamp fallback, truncation, waiting-for-input, fast ordered bursts, and control-character safety.

Graph impact: CLI stream rendering consumes daemon stream items and lifecycle terminal formatting. It should not change daemon stream subscription behavior or worker event emission.

## Coupled files

- `packages/daemon/src/service.rs`: provides `occurred_at` in proto stream events.
- `packages/daemon/src/registry.rs` and `packages/daemon/src/event.rs`: preserve sequence ordering and timestamp metadata.
- `packages/worker/src/runtime.rs`: later changes repo-preparation message to `cloning repo: <repo-url>`.
- `packages/lifecycle/src/terminal.rs`: source of timestamped safe line rendering.

Graph impact: event ordering remains sequence/stream order from daemon, while timestamp is display metadata. The renderer should not sort by timestamp or add nested per-event fields.

## Ownership

- Intended worker write scope: CLI lifecycle handler/client/output code and CLI lifecycle output tests.
- Read-only context: daemon timestamp conversion, registry replay/tail behavior, worker runtime event names/messages.
- Known conflict risks: previous CLI effort may already touch the same files; this effort starts only after effort 05 is complete and committed.

## Tests to add or update

- Worker stream prints `[<timestamp>] started` as the first lifecycle event line.
- Stream events render as one timestamped line each in daemon-observed order.
- Clone activity renders as `cloning repo: <repo-url>` when that event message arrives.
- `occurred_at` is used when present; render-time fallback is used when absent.
- History truncation renders as one timestamped line.
- Waiting-for-input renders as one timestamped line with request identity when useful, without extra un-timestamped helper lines.
- Fast event bursts remain line-oriented and ordered.
- Newline, carriage return, ESC/control characters, and secrets cannot forge extra lifecycle lines or leak credentials.

## Regression suites

- `cargo test -p cli lifecycle_output`
- `cargo test -p daemon service`
- `cargo test -p daemon --test lifecycle_service`
- `cargo test -p lifecycle`

Graph impact: CLI suite validates stream formatting; daemon suites protect replay/live ordering and timestamp conversion; lifecycle suite protects safe rendering used by the stream.

## Acceptance criteria

- Stream output starts with `[<timestamp>] started` unless the existing stream command flow explicitly requires connection lines before stream events.
- Each subsequent source event renders as one complete timestamped safe terminal line.
- Clone activity is displayed with corrected spelling when the event message contains `cloning repo: <repo-url>`.
- History truncation, waiting-for-input, failure, success, stop, and comparable lifecycle events remain concise and line-oriented.
- CLI uses event `occurred_at` when available and render-time fallback when absent.
- No old `[stream] Lifecycle worker ...` header or nested event block remains in worker stream output.
- Stream rendering does not change daemon stream semantics, event capacity, or ordering.

## Notes

- Do not add separate stream connection lifecycle lines unless existing command flow requires them. The PRD default is that stream lifecycle begins with `started`.
