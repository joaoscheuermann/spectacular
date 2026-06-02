# Technical Grounded Evaluation

Receipt id: technical_grounded_evaluator_01
Role: technical grounded evaluator
Phase goal: Evaluate the current TDD against the repository's actual architecture, package boundaries, APIs, test tooling, and coding conventions.

## Decision

Pass.

The current `TDD.md` fits the observed repository architecture closely enough to proceed to the technical assumption evaluator. I did not find a blocking architecture gap that requires a fresh architect repair pass or `GAPS_REPORT.md`.

## Grounded Architecture Assessment

The proposed dependency direction is valid.

- `docs/architecture-and-packages.md` identifies `cli` as the `doric` binary and composition root, `lifecycle` as the shared lifecycle domain/proto package, `daemon` as the lifecycle gRPC server and worker registry/session owner, and `worker` as the daemon-controlled runtime.
- The current manifests match that direction: `cli` depends on `daemon` and `lifecycle`; `daemon` depends on `lifecycle` and `config`; `worker` depends on `lifecycle`, `agent`, `config`, `llms`, and `tools`; `lifecycle` does not depend on higher-level packages.
- The TDD does not propose a reverse dependency, durable schema, new daemon API, new CLI command, or protobuf change. That is consistent with the current architecture and the existing optional `WorkerSummary.updated_at` and `WorkerEvent.occurred_at` proto fields.

The proposed package ownership is mostly accurate.

- `lifecycle` is the right place for shared identity, repo validation/redaction, event contracts, and pure safe-line helpers because `cli`, `daemon`, and `worker` already consume lifecycle contracts.
- `daemon` is the right owner for dispatch validation ordering, registry/session state, event timestamps, worker launch/session logging, and production worker ID generation.
- `cli` is the right owner for visible command sequences, connection lifecycle output, worker tables, and stream rendering.
- `worker` is the right owner for clone-start event emission because `packages/worker/src/runtime.rs` receives `StartJob.repo` and sends `repo_preparation` before invoking the repo preparer.

One ownership nuance is non-blocking: keep any shared lifecycle terminal helper plain and pure. It should not own terminal styling, table layout, stream IO, stdout/stderr choice, or CLI-specific wording beyond reusable safe message and `[timestamp] message` formatting.

## Existing API Seams And Required Refactors

The TDD correctly identifies the CLI connection seam as a required refactor.

- `packages/cli/src/main/lifecycle.rs` currently formats dispatch/list output only after `client.dispatch` or `client.list` returns.
- `packages/cli/src/main/lifecycle_client.rs` currently calls `connect_client` inside each RPC helper. That cannot produce `connected to daemon...` before `creating worker...` unless the client interface exposes an explicit connection boundary or a progress callback.
- The existing `LifecycleDaemonClient` fake in `packages/cli/tests/unit/lifecycle_output.rs` is a good test seam, but it will need to model connection separately from RPC calls.

The daemon seams are feasible.

- `LifecycleService` is already constructed through `DispatchDeps`, with explicit `registry`, `launcher`, `command_sender`, and `id_generator`.
- `SessionManager` already receives `WorkerSessionConfig`, so adding a daemon lifecycle logger there is an additive dependency-injection change.
- `LifecycleService::dispatch` currently validates mode/prompt/repo/root, builds a `RepoIdentity`, allocates a worker ID, prepares layout, inserts the registry record, appends the accepted event, then launches. The TDD's requirement to reject path-like repo input before ID allocation, layout creation, registry insert, accepted event, or launch is feasible in this order.

The timestamp refactor is real and accurately scoped.

- `Registry` and `RegistryWorkerEvent` currently have sequence numbers but no timestamp fields.
- `LifecycleService::list` currently sets `WorkerSummary.updated_at` to `None`.
- `worker_event_to_proto` and `truncated_to_proto` currently set `WorkerEvent.occurred_at` to `None`.
- Adding timestamps to registry records/events and populating existing proto fields is compatible with the current protobuf contract.

The worker event wording change is feasible.

- `run_worker_session` currently sends `WorkerEvent::repo_preparation(..., "Preparing repository for prompt requirements")` before `repo_preparer.prepare(...)`.
- `RuntimeRepoRequest` carries the raw repo URL to Git clone. The implementation must render clone-start display text from a redacted repo identity while preserving raw repo only for clone execution.

## Dependency Feasibility

`uuid` is feasible but needs a sharper implementation note.

- `uuid` is not a current direct workspace dependency.
- `cargo info uuid` resolved `uuid 1.23.2`, rust-version `1.85.0`; the local toolchain is `rustc 1.94.0`.
- The crate exposes `v6`, `rng`, and `std` features, and `Uuid::now_v6` exists.
- Important detail: `Uuid::now_v6` takes a `&[u8; 6]` node id. Decomposition should require a process-owned node id decision, deterministic fixtures for tests, and assertions that generated IDs are version 6. The existing `IdGenerator` seam can carry this state, so this is not blocking.

`url` parsing is feasible but not sufficient by itself.

- `url 2.5.8` is available and compatible, but it parses scheme URLs, not SCP-like Git remotes such as `git@github.com:org/repo.git`.
- If SCP-like remotes remain accepted, implementation needs a small custom parser path alongside `url::Url`.
- The validation contract should explicitly reject local relative paths, absolute POSIX paths, Windows drive paths, UNC paths, `file://`, cannot-be-a-base URLs, blank hosts, and local-looking values before worker creation.

Timestamp formatting is feasible with a direct dependency decision.

- `chrono` is already a direct dependency of `cli` and `tools`, but not of `lifecycle`, `daemon`, or `worker`.
- If the shared lifecycle formatter uses chrono-style RFC3339 formatting, add `chrono` directly to `lifecycle` rather than relying on `cli`.
- If the implementation wants fewer dependencies, it can keep proto timestamp population in `daemon` via `prost-types`/system time and use a small lifecycle formatting helper with explicit tests.

## Test Tooling And Placement

The TDD's testing plan matches repository tooling and seams.

- Package `project.json` files expose Rust `build`, `test`, and `lint` targets through `@monodon/rust`.
- Tests are under package `tests/` directories. CLI, daemon, and lifecycle source modules use the repository-allowed Rust exception of `#[cfg(test)]` include harnesses that include files from `tests/`.
- Existing test seams are strong enough for this feature:
  - `packages/cli/tests/unit/lifecycle_output.rs` has fake lifecycle clients and output assertions.
  - `packages/daemon/tests/unit/service.rs` already asserts invalid dispatch inputs produce no registry writes or launches.
  - `packages/daemon/tests/unit/registry.rs` covers insertion order, event sequence, replay, truncation, input requests, and answers.
  - `packages/daemon/tests/unit/worker_session.rs` covers attach/status/event/session behavior.
  - `packages/worker/tests/unit/runtime.rs` and `packages/worker/tests/unit/repo.rs` cover repo preparation events, clone command construction, and redaction.
  - `packages/lifecycle/tests/unit/redaction.rs` and `packages/lifecycle/tests/unit/domain.rs` are the right home for repo validation, safe-line, and ID behavior tests.

Decomposition should ensure red tests are placed in the owning package before implementation, especially for URL-only validation, safe one-line rendering, UUIDv6 generation, connection output order, timestamp propagation, and daemon logger coverage.

## Security, Privacy, And Operations Fit

The security/privacy plan is aligned with current trust boundaries.

- Raw repository input crosses CLI, daemon API, worker start job, Git clone, daemon event stream, CLI table output, and daemon terminal logs.
- Existing redaction removes scheme URL userinfo and some provider/API-key-like tokens, but current code does not enforce URL-only repo input and does not sanitize CR/LF/ESC/control characters as a general terminal-integrity boundary.
- The TDD correctly requires both CLI-side and daemon-side validation, redacted display identities, safe one-line terminal rendering, and no unnecessary echoing of rejected local paths.

The operational plan is compatible with current runtime behavior.

- Registry event retention is already bounded per worker and ordered by sequence.
- Rendering one line per lifecycle event is O(message length) and does not require dynamic redraws, JSON logs, or durable audit storage.
- Writing daemon lifecycle logs to stderr is a reasonable default because the daemon is a terminal process and stdout is not currently needed for this feature's protocol surface.

## Blocking Gaps

None.

No grounded finding requires a fresh architect repair pass. `GAPS_REPORT.md` is not required for this evaluator result.

## Non-blocking Findings

1. UUIDv6 generation needs an explicit node-id plan. `Uuid::now_v6(&node_id)` exists, but the node id must be process-owned or otherwise intentionally generated. Keep it behind the existing `IdGenerator` seam and test UUID version 6 directly.
2. URL validation syntax needs to be locked during decomposition. The TDD's accepted forms are reasonable, but SCP-like Git remotes require custom parsing beyond the `url` crate.
3. Shared timestamp formatting needs a direct dependency decision. If lifecycle owns formatting, add the formatter dependency to `lifecycle`; do not couple lifecycle formatting to `cli`.
4. Keep raw repo input tightly scoped to worker launch and Git clone. Registry summaries, daemon logs, CLI tables, stream lines, and errors should use redacted safe display values.
5. Daemon logging should be injected into both `LifecycleService` and `SessionManager` and should log after successful registry/session state changes. Avoid registry-level observer complexity unless later tests prove service/session boundaries miss required categories.
6. Global `WorkerId::from_str` strict UUID parsing is not required for this PRD. Creation-boundary UUIDv6 generation is sufficient for newly created workers; strict parsing would add broad fixture churn without a clear product benefit.
7. The `debug` dispatch command is outside the explicit product output change. If implementation reuses the new lifecycle sequence for `debug`, tests should make that intentional rather than accidental.

## Recommended Next Step

Proceed to the technical assumption evaluator. Carry the non-blocking findings above into Dependency Hop verification and decomposition notes, but do not send the TDD back for repair on grounded architecture fit.
