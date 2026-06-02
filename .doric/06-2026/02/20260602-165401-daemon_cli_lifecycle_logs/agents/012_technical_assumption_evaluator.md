# Technical Assumption Evaluation

Receipt id: technical_assumption_evaluator_01
Role: technical assumption evaluator
Phase goal: Independently verify every Dependency Hop in the current TDD and determine whether the design is ready for user approval to proceed to decomposition.

## Decision

Pass.

The current `TDD.md` is ready for coordinator review and user approval to proceed to decomposition. I found no invalid Dependency Hops and no blocking architecture gaps requiring a fresh architect repair pass. `GAPS_REPORT.md` is not required.

The design is feasible against the current source. Decomposition must preserve the evaluator caveats below as concrete effort constraints, especially around UUIDv6 node-id selection, SCP-like Git remote parsing, timestamp dependency placement, raw repository URL scoping, daemon logger injection, WorkerId parser scope, and debug output scope.

## Evidence inspected

- Product and architecture source artifacts: `PROMPT.md`, `PRD.md`, and `TDD.md`.
- Prior evaluator artifacts: `agents/010_technical_initial_filter.md` and `agents/011_technical_grounded_evaluator.md`.
- Doric technical-design contract: `.agents/skills/doric/references/03-technical-design.md`.
- Coding and architecture standards: `.agents/skills/coding-conventions/SKILL.md`, architecture, simplicity, implementation, and Rust references; `.agents/skills/current-architecture/SKILL.md`; `docs/architecture-and-packages.md`.
- Source and tests: `packages/lifecycle`, `packages/cli`, `packages/daemon`, and `packages/worker` lifecycle, service, registry, runtime, repo, proto, manifest, and test files.
- Dependency feasibility checks: `cargo info uuid`, `cargo info url`, `cargo info chrono`, `cargo metadata --no-deps --format-version 1`, and local registry source for `uuid-1.23.2`.

## Dependency Hop Assessment

### 1. Shared terminal lifecycle line formatter

- Assumption support: Supported. The architecture document places shared lifecycle domain/event contracts, repo identity parsing, and redaction helpers in `lifecycle`. `cli`, `daemon`, and `worker` already depend on `lifecycle`, while `lifecycle` does not depend upward.
- Dependency existence or feasibility: Feasible. `packages/lifecycle/src/redaction.rs` and `packages/lifecycle/src/repo.rs` already provide shared redaction and repo display identity. A new pure `terminal` or equivalent module can be added without pulling CLI styling or daemon IO into `lifecycle`.
- Failure mode and fallback adequacy: The TDD correctly identifies the divergence risk if CLI and daemon duplicate timestamp/safe-line formatting. The fallback of sharing only safe-message/repo redaction while keeping timestamp formatting local is adequate only if tests prove equivalent output.
- Readiness: Ready for decomposition. Decomposition should choose whether lifecycle uses `chrono` directly or a smaller std/prost-time formatter and should test exact `[timestamp] message` behavior plus safe one-line rendering.

### 2. Explicit CLI connection lifecycle

- Assumption support: Supported. `LifecycleDaemonClient` currently exposes `dispatch`, `list`, `stream_worker`, and `answer`, and `GrpcLifecycleDaemonClient` connects inside each RPC helper through `connect_client`. `handle_dispatch` and `handle_list` format output only after RPC completion.
- Dependency existence or feasibility: Feasible. The CLI already has injected fake clients in `packages/cli/tests/unit/lifecycle_output.rs`, so a connected-client/session trait or progress callback can be tested without network.
- Failure mode and fallback adequacy: The TDD correctly states that output will otherwise be ordered too late. The progress-callback fallback is feasible, but explicit connection is the clearer deep-module boundary.
- Readiness: Ready for decomposition. Tests must assert connection, connected, action, and RPC ordering.

### 3. URL-only repository validation

- Assumption support: Supported with caveat. Current `RepoIdentity::from_raw_url` delegates to `redact_repo_url`, which accepts non-blank values without `://`; therefore path-like values currently pass. This confirms the need for a stricter shared repo URL type.
- Dependency existence or feasibility: Feasible. `url 2.5.8` is compatible with the local toolchain and can parse scheme URLs. SCP-like Git remotes are not standard URLs and require a small custom parser path.
- Failure mode and fallback adequacy: The TDD correctly identifies local paths reaching worker creation or valid SSH remotes being rejected. The fallback of accepting explicit scheme URLs plus bounded `git@host:path` syntax is adequate if validation errors document the allowed forms.
- Readiness: Ready for decomposition. Decomposition must lock accepted syntax before implementation and include red tests for explicit scheme URLs, SCP-like remotes, relative paths, POSIX absolute paths, Windows drive paths, UNC paths, blank hosts, cannot-be-a-base URLs, and `file://`.

### 4. Daemon-side enforcement before side effects

- Assumption support: Supported. `LifecycleService::dispatch` currently parses mode, prompt, repo, validates root, builds repo identity, allocates a worker id, prepares layout, inserts the registry record, appends accepted event, then launches. Moving strict repo validation before ID allocation, layout creation, registry insert, event append, and launch fits the current flow.
- Dependency existence or feasibility: Feasible. Existing daemon service tests already assert blank prompt/repo/mode/root failures leave registry and launcher untouched. The `IdGenerator`, `WorkerLauncher`, and registry seams are injectable.
- Failure mode and fallback adequacy: The TDD names the correct failure: rejected path-like input could still allocate IDs, create directories, or appear in logs/tables. The targeted daemon tests are adequate.
- Readiness: Ready for decomposition. Prefer validating strict repo syntax before worker root validation too, so invalid repo input produces the intended URL-only error even if the daemon root is misconfigured.

### 5. UUIDv6 production worker IDs

- Assumption support: Supported. Production currently uses `TimestampIdGenerator` to produce `feature-<nanos>` or `debug-<nanos>`, while tests inject deterministic IDs through `IdGenerator`. This is the right seam to replace production generation.
- Dependency existence or feasibility: Feasible. `cargo info uuid` resolves `uuid 1.23.2` with rust-version `1.85.0`; local `rustc` is `1.94.0`. Local crate source confirms `Uuid::now_v6(node_id: &[u8; 6])` and `Uuid::new_v6(ts, node_id)`. `now_v6` is gated by `v6`, `std`, and `rng`.
- Failure mode and fallback adequacy: The TDD correctly identifies non-UUID or non-sortable IDs as failure. The existing generator seam makes `new_v6` with explicit timestamp/context a good fallback if `now_v6` is not selected.
- Readiness: Ready for decomposition. The node ID must be process-owned and intentional. Tests should assert UUID version 6, not only string shape.

### 6. Worker ID path safety

- Assumption support: Supported for production IDs. UUIDv6 strings are safe single path components. Current daemon layout joins `root/<worker-id>` directly and relies on generator safety; worker repo layout separately validates path-like worker IDs and has tests for escape attempts.
- Dependency existence or feasibility: Feasible. The existing `IdGenerator` seam can continue to generate safe production IDs, and daemon-side single-component validation can be added if desired.
- Failure mode and fallback adequacy: The TDD correctly identifies malformed injected IDs escaping the daemon worker root as the risk. The fallback of adding daemon-side component validation before layout creation is adequate.
- Readiness: Ready for decomposition. This is not a blocking PRD issue because callers cannot supply worker IDs at creation, but adding daemon-side component validation would be a small hardening task.

### 7. Event timestamps without proto change

- Assumption support: Supported. `v1.proto` already contains `WorkerSummary.updated_at` and `WorkerEvent.occurred_at`. Current daemon conversions set both to `None`, registry records/events have no timestamp fields, and the current CLI DTOs do not retain timestamp values.
- Dependency existence or feasibility: Feasible. `lifecycle` already depends on `prost-types`; `daemon` can populate existing proto timestamp fields from registry mutation time. CLI rendering can use event timestamps when present and render time as compatibility fallback.
- Failure mode and fallback adequacy: The TDD correctly names replay timestamp loss. Render-time fallback is adequate for absent legacy/test events, but not as the final implementation for retained daemon events if timestamped stream output is required.
- Readiness: Ready for decomposition. Decomposition should include timestamp plumbing in daemon registry/events and CLI DTOs, not only formatting at render time.

### 8. Daemon terminal logs through an injected logger

- Assumption support: Supported. `LifecycleService` sees dispatch and stream start; `SessionManager` sees attach, status updates, worker events, input requests, answer forwarding, spawn failures, attach deadline failures, and child exits. These are the right boundaries for the required daemon lifecycle categories.
- Dependency existence or feasibility: Feasible. Both service and session manager already receive dependency/config structs, so adding a daemon-local `LifecycleLogger` dependency is an additive injection change.
- Failure mode and fallback adequacy: The TDD correctly identifies missed lifecycle categories or logs before failed state changes. The fallback of moving logging closer to `Registry::append_event` and `Registry::update_status` is adequate if service/session tests prove gaps.
- Readiness: Ready for decomposition. Logger calls should happen after successful state mutation or after known failure mapping, and tests should use recording/noop loggers rather than real terminal assertions.

### 9. Clone-start event text

- Assumption support: Supported. `worker` receives `StartJob.repo` and builds `RuntimeRepoRequest` from it. `run_worker_session` currently emits a `repo_preparation` event before invoking the repo preparer.
- Dependency existence or feasibility: Feasible. `worker` already depends on `lifecycle`, so it can reuse repo identity/redaction helpers for display while preserving raw repo input for Git clone.
- Failure mode and fallback adequacy: The TDD correctly identifies missing clone activity or raw credential leakage. The fallback daemon-side event is possible but less accurate if the worker never attaches.
- Readiness: Ready for decomposition. Implement clone-start wording in worker runtime with redacted display repo and raw clone URL retained only for repo preparation.

### 10. Safe one-line terminal output

- Assumption support: Supported with implementation gap. Existing redaction handles scheme URL userinfo and some provider/API-key-like tokens, and worker runtime redacts one-time tokens. No shared lifecycle helper currently guarantees CR/LF/ESC/control-character replacement for all terminal surfaces.
- Dependency existence or feasibility: Feasible. A pure lifecycle safe-line helper can be applied by CLI renderers and daemon loggers, with worker-local token redaction kept in `worker`.
- Failure mode and fallback adequacy: The TDD correctly names forged lifecycle lines, terminal escape output, and secret leakage. Making safe-line rendering mandatory at every terminal write plus tests for newline, carriage return, ESC, and credential-bearing fixtures is an adequate mitigation.
- Readiness: Ready for decomposition. This must be treated as a required rendering boundary, not optional polish.

### 11. Worker list table preserves order

- Assumption support: Supported. `Registry` stores an `order: Vec<WorkerId>`, pushes IDs on insert, and `Registry::list` iterates that order. The CLI currently preserves response vector order while rendering rows.
- Dependency existence or feasibility: Feasible. The new CLI table can compute widths over the incoming vector without sorting.
- Failure mode and fallback adequacy: The TDD correctly identifies accidental sorting as the primary risk. The fallback of not sorting and computing widths from the ordered vector is adequate.
- Readiness: Ready for decomposition. Tests should make the known daemon order observable in the rendered table.

### 12. Fast event burst readability

- Assumption support: Supported. Registry event retention is bounded, event replay is sequence ordered, live subscribers receive one `RegistryWorkerEvent` at a time, and server streaming replays retained events before live events.
- Dependency existence or feasibility: Feasible. Existing CLI worker stream code writes items as they arrive and has tests for replay order, live first-event visibility, and truncation notices.
- Failure mode and fallback adequacy: The TDD correctly identifies nested/noisy output and confusing truncation. Keeping history-truncated as one timestamped line and avoiding per-event metadata columns is adequate.
- Readiness: Ready for decomposition. Add tests for burst-like inputs rendering one complete timestamped line per event.

## Grounded Evaluator Finding Disposition

- UUIDv6 node-id plan: Non-blocking, but real. The process-owned node ID must be decided in decomposition. Existing `IdGenerator` is the right home.
- SCP-like Git remote parsing: Non-blocking, but `url` alone is insufficient. Decomposition must specify a small custom parser for `git@host:path` or narrow the accepted syntax explicitly.
- Timestamp dependency: Non-blocking. `chrono` exists in `cli` and `tools`, not `lifecycle`; `lifecycle` should add its own dependency if it owns formatting, or use a smaller formatter based on std/prost types.
- Raw repo scoping: Non-blocking, but critical. Raw repo input should remain limited to worker launch, `StartJob.repo`, `RuntimeRepoRequest`, and Git clone. Registry summaries, logs, tables, streams, and errors should use redacted safe display values.
- Daemon logger injection: Non-blocking and feasible. Inject into `LifecycleService` and `SessionManager`; use registry-level observer only if tests prove missed categories.
- WorkerId parser strictness: Non-blocking. Current `WorkerId::from_str` accepts any non-blank string. Strict global UUID parsing is not required for the PRD's newly-created-worker scope and would create broad fixture churn. Production generation plus optional daemon path-component validation is sufficient.
- Debug output scope: Non-blocking. `debug` currently shares dispatch rendering with `feature`, but the PRD only requires replacing `feature` command output. If implementation applies the new lifecycle sequence to `debug`, tests should record that as intentional.

## Invalid Dependency Hops

None.

## Blocking Gaps

None.

No `GAPS_REPORT.md` was written.

## Recommended Next Step

Coordinator should accept this evaluator result, present the finalized TDD summary to the user, and request explicit `tdd_to_decomposition` approval before starting decomposition.
