# Effort: CLI feature and list lifecycle output

Status: todo

## Requirement links

- FEATURES.md: F5, F6, F8.
- PRD.md: FR1, FR2, FR3, FR5, FR6, FR7, FR14, FR15, FR16, FR18, FR19, FR20, FR21, FR22, FR23; User Value Hops 1, 2, 3, 5, and 8.
- TDD.md: Proposed design component 6; Dependency Hops 2, 10, and 11; CLI contracts; testing strategy CLI feature/list cases.

## Goal

Refactor CLI lifecycle command handling to expose connection success before action RPCs, then render the approved `feature` lifecycle sequence and worker/agent list connection lines plus simple table.

## Sequence

- Position: 05 of 07.
- Previous effort: 04_daemon_timestamps_logger.md.
- Next effort: 06_cli_stream_output.md.
- Dependency reason: CLI output should consume shared terminal rendering, repo validation, UUIDv6 daemon IDs, and daemon timestamp/list fields already established by earlier efforts.

## Target files

- `packages/cli/src/main/lifecycle.rs`: validate prompt/repo before connection for `feature`; write connection/action lifecycle lines in order; preserve debug behavior unless intentionally changed.
- `packages/cli/src/main/lifecycle_client.rs`: add an explicit connected-client/session seam or progress boundary so handlers can print `connected` before dispatch/list RPCs.
- `packages/cli/src/main/lifecycle_output.rs`: replace feature accepted-worker block with lifecycle-line helpers and render list table headers/rows.
- `packages/cli/src/main/cli_types.rs`: update help text from repository path to repository URL if needed.
- `packages/cli/tests/unit/lifecycle_output.rs`: update feature/list handler tests, fake client call ordering, invalid repo pre-connection behavior, redaction, table, empty-state, and ordering cases.
- `packages/cli/tests/debug_log_startup_test.rs`: update only if user-facing lifecycle command behavior changes stderr/stdout expectations.

Graph impact: CLI command handler owns user-visible sequencing. It depends on lifecycle terminal/repo helpers and daemon client responses, while `LifecycleDaemonClient` becomes the boundary that separates connection from dispatch/list actions.

## Coupled files

- `packages/lifecycle/src/terminal.rs` and `packages/lifecycle/src/repo.rs`: source of shared safe rendering and pre-connection repo validation.
- `packages/daemon/src/service.rs`: supplies UUIDv6 worker ID, redacted repo identity, and list order.
- `packages/daemon/src/registry.rs`: preserves list order that CLI must not sort.
- `packages/cli/src/main/entry.rs` and `packages/cli/src/main/output.rs`: read-only unless command error routing needs a tiny update.

Graph impact: `feature` invalid repo input should stop at CLI validation with no edge to daemon connection. Successful `feature` and `list` flow from CLI validation to explicit connection to RPC to terminal renderer.

## Ownership

- Intended worker write scope: CLI lifecycle handler/client/output/types and CLI lifecycle/debug-log tests if needed.
- Read-only context: lifecycle repo/terminal modules, daemon service/list behavior, debug-log startup path.
- Known conflict risks: `debug` currently shares dispatch handling with `feature`. Any `debug` output change must be intentional, tested, and called out.

## Tests to add or update

- `feature` with a valid URL prints connecting, connected, creating worker, and created worker lines in order.
- `feature` output includes a UUIDv6 worker ID returned by the client and omits old `[accepted] Lifecycle worker`, `Mode`, `Repo`, `Status`, and `Scope` accepted-worker detail.
- `feature` invalid local/path-like repo rejects before client connection or dispatch and returns an understandable URL-only message.
- `list` prints connection lifecycle lines before a simple table with `status`, `worker id`, and `git repo` headers.
- `list` preserves daemon-provided order without sorting.
- Empty list remains table-like or uses the established empty representation while preserving required headers and connection lines.
- Repo credentials and control characters are safe in feature/list output.

## Regression suites

- `cargo test -p cli lifecycle_output`
- `cargo test -p cli --test debug_log_startup_test`
- `cargo test -p lifecycle`
- `cargo test -p daemon service`

Graph impact: CLI tests validate output and connection sequencing, lifecycle tests validate shared safe rendering and validation, daemon service tests protect the response/order contract consumed by CLI.

## Acceptance criteria

- Successful `feature` prints exactly the required connection and worker creation lifecycle sequence in order.
- Successful `feature` no longer prints verbose accepted-worker details.
- CLI validates prompt and repo before connection; invalid repo values create no daemon connection or dispatch call.
- Worker/agent listing prints connection lifecycle lines followed by a simple table with status, worker id, and git repo.
- Listing preserves daemon order and handles empty state clearly.
- Output uses shared safe lifecycle rendering and redacts credentials.
- `debug` output is unchanged unless deliberately touched; if touched, tests and notes make the change intentional.

## Notes

- Keep table rendering low-noise and plain. Do not introduce rich TUI redraws, progress spinners, or nested cards/blocks.
