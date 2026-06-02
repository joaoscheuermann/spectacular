# Technical Design Document

## Summary

Implement daemon and CLI lifecycle visibility as a line-oriented terminal output layer on top of the existing lifecycle service, registry, worker-session, and worker-runtime flow.

The design keeps package ownership aligned with the current architecture:

- `lifecycle` owns shared lifecycle identity, repository validation/redaction, event contracts, timestamped line formatting, and safe one-line message rendering.
- `daemon` owns worker creation, registry/session state, daemon terminal logging, event timestamps, and production UUIDv6 worker ID generation.
- `cli` owns user-facing command output, connection lifecycle lines, worker tables, and event stream rendering.
- `worker` owns repo clone startup event emission and worker-local runtime events.

No durable storage, schema migration, new daemon API, new CLI command, JSON log format, rich TUI rendering, or full tracing system is required. The gRPC protobuf already contains optional timestamp fields for worker summaries and events, so the implementation can populate existing contract fields rather than changing the IDL.

## Current architecture context

The repository architecture document identifies `cli` as the `doric` binary and application composition root, `lifecycle` as the shared lifecycle domain/proto package, `daemon` as the lifecycle gRPC server and worker registry/session owner, and `worker` as the daemon-controlled runtime that prepares repositories and emits worker events.

Observed implementation evidence:

- `packages/cli/src/main/lifecycle.rs` dispatches lifecycle commands through a `LifecycleDaemonClient` trait and currently returns formatted command output after RPC calls.
- `packages/cli/src/main/lifecycle_output.rs` currently renders verbose accepted-worker summaries, list rows, and worker stream rows. This is the primary CLI formatting surface to replace for `feature`, list, and stream output.
- `packages/cli/src/main/lifecycle_client.rs` currently connects inside each RPC helper, so connection lifecycle output cannot be emitted in the required order without introducing an explicit connection boundary or progress callback.
- `packages/daemon/src/service.rs` validates dispatch input, creates worker records, appends the accepted event, launches workers, lists registry summaries, and converts registry events to protobuf.
- `packages/daemon/src/registry.rs` preserves worker insertion order and maintains a bounded per-worker event log, which supports the PRD requirement that worker listing preserve existing order and streams stay line-oriented under event bursts.
- `packages/daemon/src/worker_session.rs` records worker attach, status updates, worker-emitted events, input requests, answer forwarding, terminal states, attach-deadline failures, spawn failures, and child exits.
- `packages/worker/src/runtime.rs` emits `repo_preparation`, prompt-agent, failure, success, stop, and input events through the daemon session.
- `packages/lifecycle/src/identity.rs` currently treats `WorkerId` as any non-blank string; production daemon IDs currently come from `TimestampIdGenerator` in `packages/daemon/src/service.rs` as `feature-<nanos>` or `debug-<nanos>`.
- `packages/lifecycle/src/repo.rs` and `packages/lifecycle/src/redaction.rs` currently redact credential-bearing scheme URLs but do not enforce URL-only repository input. Values without `://` are accepted as identities today.
- `packages/lifecycle/proto/doric/lifecycle/v1.proto` already includes `WorkerSummary.updated_at` and `WorkerEvent.occurred_at`; current conversions set both to `None`.
- Existing tests live under package-level `tests/` directories and use injected clients, launchers, command senders, fake Git runners, and runtime fakes, matching the coding-conventions test-location and dependency-injection rules.

Coding-standard implications:

- Keep formatter, repo validation, and ID semantics as deep shared lifecycle modules rather than duplicating string logic across CLI, daemon, and worker.
- Keep side effects at command/service/session boundaries; pure rendering and validation should be unit-testable without daemon or network processes.
- Add abstractions only where there is a concrete second consumer. Shared terminal line formatting has at least daemon and CLI consumers; daemon-only logging should stay daemon-owned.

## Technical persona debate

- Database Administrator: There is no persistent database or durable audit log. The important data-integrity concern is in-memory event consistency: event timestamps, sequence numbers, worker IDs, repo identities, and summary state must be updated together after successful state transitions. This persona wins on using existing proto timestamp fields and registry insertion order instead of a separate log store.
- Frontend Lead: The CLI is the frontend here. The required output must be short, stable, and cheap to scan. This persona wins on replacing verbose accepted-worker blocks with timestamped lines, rendering worker list rows as a simple table, and reducing worker stream output to one safe line per source event.
- SecOps Specialist: Repository values and event messages cross trust boundaries from user input, daemon API callers, Git stderr, worker runtime, and prompt-agent events into terminal output. This persona wins on URL-only validation at both CLI and daemon boundaries, credential redaction before every terminal surface, and line/control-character sanitization to prevent terminal output forging.

Accepted tradeoffs:

- Populate in-memory timestamps and render timestamped lines instead of adding durable logs. This satisfies operator visibility without expanding scope into audit storage.
- Use shared lifecycle rendering and validation for daemon/CLI consistency, but keep worker token-specific redaction in `worker` because only the worker runtime knows the session token.
- Guarantee UUIDv6 for newly created production workers at the daemon creation boundary. Keep inbound parsing compatibility for existing string references unless evaluators require global parser tightening; production display surfaces will receive UUIDv6 IDs because the daemon generates them.
- Refactor the CLI lifecycle client enough to expose connection state. This is a contained interface change that avoids printing `connected` only after worker creation has already completed.

## Proposed design

1. Add shared lifecycle terminal output primitives.
   - Add a small lifecycle module for terminal-safe lifecycle lines, for example `packages/lifecycle/src/terminal.rs`.
   - Provide:
     - a compact RFC3339/ISO-style UTC timestamp formatter using the same style already used by chat session timestamps when practical,
     - `format_line(timestamp, message) -> String` that returns `[<timestamp>] <safe-message>`,
     - `safe_message(raw) -> String` that redacts known secrets, removes or replaces CR/LF and terminal control characters, and returns a single display line,
     - repo-display helpers that combine repository redaction with safe one-line rendering.
   - Keep color/styling decisions in `cli`; the shared lifecycle formatter should produce plain text.

2. Add strict clone repository validation in `lifecycle`.
   - Introduce a shared domain type such as `RepoUrl` or a stricter constructor on `RepoIdentity`.
   - Accept cloneable remote URL syntax only:
     - scheme URLs with non-empty host for allowed remote schemes such as `https`, `http`, `ssh`, and `git`,
     - SCP-like Git remote syntax such as `git@github.com:org/repo.git` when host and remote path are present.
   - Reject local relative paths, absolute POSIX paths, Windows drive/UNC paths, path-like values, empty values, and local `file://` inputs.
   - Keep raw clone input available only for `git clone`; keep redacted identity available for registry summaries, daemon logs, CLI tables, and stream messages.
   - CLI command handling must validate before connecting to the daemon so path-like input errors do not open a daemon connection.
   - Daemon dispatch must perform the same validation before worker ID allocation, registry insertion, layout creation, or worker launch to protect non-CLI callers.

3. Generate UUIDv6 worker IDs at daemon dispatch.
   - Replace production `TimestampIdGenerator` with a UUIDv6 generator.
   - Prefer the Rust `uuid` crate with `v6`, `std`, and RNG support. Current docs.rs evidence for the crate exposes `Uuid::now_v6` and notes the `v6`, `std`, and `rng` feature requirements.
   - Keep the daemon `IdGenerator` seam for deterministic tests, but have production wiring generate UUIDv6 strings and immediately wrap them in `WorkerId`.
   - Use UUIDv6 fixture values in tests for newly-created-worker flows.
   - Consider a follow-up evaluator decision on whether `WorkerId::from_str` should reject non-UUID values globally. The recommended implementation does not require that broad parser change to satisfy the PRD because callers cannot supply worker IDs during creation and the registry is in-memory.

4. Populate event timestamps in the daemon registry path.
   - Extend daemon registry records and events with an `updated_at` or `occurred_at` value captured at successful state mutation time.
   - Populate `WorkerSummary.updated_at` and `WorkerEvent.occurred_at` in `packages/daemon/src/service.rs` conversions.
   - CLI rendering should use `occurred_at` when present and fall back to render time when replayed legacy/test events have no timestamp.
   - This avoids a protobuf change while preserving event-time ordering for replayed streams.

5. Add daemon terminal logging through an injected daemon logger.
   - Introduce a daemon-local logging boundary, for example `LifecycleLogger`, with production `TerminalLifecycleLogger` and test `RecordingLifecycleLogger`/`NoopLifecycleLogger`.
   - Write production daemon lifecycle lines to stderr. This matches existing fatal daemon error behavior and avoids stdout becoming an accidental protocol surface.
   - Inject the logger into `LifecycleService` and `SessionManager` through existing config/dependency structs.
   - Emit only after successful state changes, not before, so logs cannot claim a worker or status exists when the registry mutation failed.
   - Covered daemon moments:
     - worker creation accepted/created,
     - worker launch failure,
     - worker attached,
     - stream started,
     - worker status update,
     - repo clone start from `repo_preparation`,
     - input requested and answer provided without answer text,
     - worker failure, success, stop, attach-deadline failure, and child exit status.

6. Refactor CLI lifecycle command output.
   - Add an explicit connection boundary around lifecycle RPCs, such as a connected client/session trait, so `feature` and `list` can write:
     - `[<timestamp>] connecting to daemon...`
     - `[<timestamp>] connected to daemon...`
   - For `feature`, validate prompt and repo first, then connect, then write `[<timestamp>] creating worker...`, call dispatch, and write `[<timestamp>] created worker: <worker-id>`.
   - Stop rendering the old accepted-worker block for `feature`.
   - Keep `debug` outside the required output change unless implementation chooses to reuse the new sequence deliberately. If reused, tests should record that decision as intentional.
   - For `list`, connect first, then render a simple table with headers for status, worker id, and git repo. Preserve the daemon list order.
   - For empty lists, keep the required headers and print an empty-state row or established empty-list marker that remains table-like and readable.
   - For worker event streaming, connect to the daemon, print `[<timestamp>] started`, and render one timestamped safe message per source stream item. Do not print the old `[stream] Lifecycle worker ...` header or nested event block.
   - Waiting-for-input events should be composed into one line with request identity if useful; do not add extra un-timestamped helper lines.

7. Update worker repo clone event wording.
   - Change the worker runtime's `repo_preparation` event from generic preparation wording to `cloning repo: <redacted-repo-url>`.
   - Use lifecycle repository identity/redaction for the displayed repo value.
   - Keep raw `start.repo` only for the Git clone request.

8. Preserve existing worker lifecycle semantics.
   - Do not change the registry's status transitions, input-answer validation, event capacity behavior, worker launch/session attach protocol, or prompt-agent runtime flow except for timestamps, log emission, ID generation, repo validation, and display text.
   - Do not add durable event storage or alter gRPC method names.

## Data model

No persistent database or durable schema is introduced.

In-memory data model changes:

- `WorkerRecord` should include a last-updated timestamp used for `WorkerSummary.updated_at`.
- `RegistryWorkerEvent` should include an occurred-at timestamp used for `WorkerEvent.occurred_at`.
- The event log remains a bounded per-worker ring with existing sequence numbers. Sequence remains the ordering source; timestamp is display metadata.
- `RepoUrl`/`RepoIdentity` should retain two views:
  - raw clone URL for worker launch and Git clone,
  - redacted identity/display URL for registry summaries, logs, tables, and stream text.
- `WorkerId` remains the shared typed ID. Production generation changes from timestamp-label strings to UUIDv6 strings.

Migration:

- None. The registry is in-memory and worker IDs are not persisted across daemon restarts in the current lifecycle implementation.
- Tests and fixtures that assert newly-created worker IDs should move to UUIDv6 fixtures or generator assertions. Descriptive non-production IDs can remain in tests that do not exercise creation requirements unless evaluator feedback requires strict parser enforcement.

## API or interface contracts

CLI contracts:

- `doric feature --prompt <text> --repo <url> [--addr <addr>]`
  - validates prompt and URL before connection,
  - prints exactly the successful lifecycle sequence required by the PRD,
  - does not print old accepted-worker detail,
  - surfaces daemon validation failures as existing `LifecycleError` messages after redaction/safe-line rendering.
- `doric list [--addr <addr>]`
  - prints connection lifecycle lines before table output,
  - renders status, worker id, and git repo columns,
  - preserves daemon order.
- `doric worker <worker-id> [--addr <addr>]`
  - connects, prints `[<timestamp>] started`, then prints one timestamped safe line per stream item,
  - renders `repo_preparation` as clone activity when the event message is `cloning repo: <repo-url>`,
  - handles history truncation as one timestamped line.

Daemon/service contracts:

- `DispatchRequest.repo` is now interpreted as a cloneable remote repository URL only. Invalid local/path-like inputs return invalid argument before worker ID allocation or registry insertion.
- `DispatchResponse.worker_id` for newly-created workers is UUIDv6.
- `DispatchResponse.repo_identity`, `WorkerSummary.repo_identity`, daemon logs, and CLI table/stream output use redacted repo identity.
- `ListWorkersResponse.workers` keeps existing registry insertion order.
- `WorkerEvent.occurred_at` and `WorkerSummary.updated_at` should be populated when available. Consumers must tolerate `None` for compatibility.

Internal Rust contracts:

- `LifecycleDaemonClient` should expose connection lifecycle explicitly enough for command handlers to render `connecting`, `connected`, and action lines in order.
- Daemon `LifecycleLogger` receives already-safe lifecycle messages or applies the shared lifecycle safe-line formatter before writing.
- Worker runtime emits clone-start event text through `WorkerEvent::repo_preparation`.

## Dependency Hops

1. Hop: Shared terminal lifecycle line formatter.
   - Assumption: CLI and daemon can share plain timestamp-plus-message formatting without pulling CLI styling or daemon process details into `lifecycle`.
   - Dependency: `lifecycle` package, current `lifecycle::redaction`, optional timestamp formatting dependency such as `chrono`, CLI and daemon callers.
   - Evidence: `docs/architecture-and-packages.md` defines `lifecycle` as shared lifecycle domain/event contracts; `cli` and `daemon` already depend on `lifecycle`; existing CLI output uses redaction from lifecycle through `redact_text`.
   - Failure mode: Formatting remains duplicated and daemon/CLI surfaces diverge in timestamp style or line sanitization.
   - Fallback: Keep timestamp formatting local to CLI/daemon but move only `safe_message` and repo redaction into `lifecycle`; evaluator should require tests proving equivalent output.

2. Hop: Explicit CLI connection lifecycle.
   - Assumption: The CLI client can be refactored to expose connection success before dispatch/list/stream RPCs without broad command rewrites.
   - Dependency: `packages/cli/src/main/lifecycle.rs`, `packages/cli/src/main/lifecycle_client.rs`, existing `LifecycleDaemonClient` trait/fakes.
   - Evidence: Current `connect_client` is internal to each RPC helper, and `handle_dispatch` currently formats output only after `client.dispatch`; this cannot produce `connected` before `creating worker` without a connection seam.
   - Failure mode: CLI output prints lifecycle lines in the wrong order or only after worker creation.
   - Fallback: Add a progress callback from client RPC helpers to handlers, but prefer explicit connection because it is easier to test and keeps output in CLI command code.

3. Hop: URL-only repository validation.
   - Assumption: The lifecycle package can define cloneable remote repository syntax precisely enough for CLI and daemon validation.
   - Dependency: New lifecycle repo validation type, existing redaction helpers, possible `url` crate for scheme URL parsing, small custom parser for Git SCP-like remotes.
   - Evidence: Current `RepoIdentity::from_raw_url` accepts values without a scheme, so daemon dispatch currently allows path-like repo input after only blank checks.
   - Failure mode: Local paths still reach worker creation, or cloneable SSH/SCP remotes are accidentally rejected.
   - Fallback: If allowed clone syntax remains ambiguous, restrict initial acceptance to explicit scheme URLs plus `git@host:path` and document the allowed forms in validation errors.

4. Hop: Daemon-side enforcement before side effects.
   - Assumption: Dispatch can validate repo URL before worker ID generation, layout creation, registry insert, accepted-event append, and worker launch.
   - Dependency: `LifecycleService::dispatch`, `RepoUrl`, `WorkerLauncher`, registry mutation order.
   - Evidence: `dispatch_empty_repo_rejects_without_record_or_launch` and related service tests already assert invalid input prevents registry writes and launch.
   - Failure mode: Rejected path-like input still allocates a worker ID, creates worker directories, or appears in worker tables/logs.
   - Fallback: Add targeted daemon tests for relative path, absolute POSIX path, Windows path, and `file://` before changing implementation.

5. Hop: UUIDv6 production worker IDs.
   - Assumption: The selected UUID dependency can generate UUIDv6 on supported Doric targets without destabilizing the workspace.
   - Dependency: `uuid` crate with v6/std/RNG feature set, `packages/lifecycle/src/identity.rs`, daemon production generator wiring in `packages/daemon/src/server.rs`.
   - Evidence: The current daemon generator produces `feature-<nanos>`/`debug-<nanos>`, which is not UUIDv6. Current docs.rs crate documentation exposes `Uuid::now_v6` with `v6`, `std`, and `rng` requirements: https://docs.rs/uuid/latest/uuid/struct.Uuid.html.
   - Failure mode: IDs are not UUIDv6, IDs are not time-sortable as required, or dependency features do not resolve on local toolchain.
   - Fallback: If `Uuid::now_v6` cannot be used after Cargo resolution, implement a small UUIDv6 generator behind the existing `IdGenerator` seam only after evaluator approval, or use `Uuid::new_v6` with explicit timestamp/context from the crate.

6. Hop: Worker ID path safety.
   - Assumption: UUIDv6 production IDs are safe as worker root path components.
   - Dependency: Daemon `prepare_worker_layout`, worker `prepare_worker_layout`, `WorkerId`, production ID generator.
   - Evidence: Worker repo tests already validate path-like worker IDs are rejected in worker layout; daemon layout currently joins `root/<worker-id>` directly and relies on generator safety.
   - Failure mode: A malformed injected worker ID escapes the worker root in daemon tests or future code.
   - Fallback: Add a daemon-side single-component validation or strict generated-ID constructor before layout creation if evaluators consider injected ID safety a blocker.

7. Hop: Event timestamps without proto change.
   - Assumption: Existing proto timestamp fields can carry lifecycle display timestamps without changing generated contract files.
   - Dependency: `WorkerSummary.updated_at`, `WorkerEvent.occurred_at`, daemon registry event/record model, `prost-types`.
   - Evidence: `v1.proto` already defines both timestamp fields, and current service conversions set them to `None`.
   - Failure mode: CLI stream replay uses render time for old events and loses event-time meaning.
   - Fallback: If registry timestamp plumbing is too large for the first effort, use render-time fallback for CLI/daemon lines and create a follow-up effort to populate proto timestamps, but evaluator should treat that as a risk against replay accuracy.

8. Hop: Daemon terminal logs through an injected logger.
   - Assumption: Service/session boundaries see all required daemon lifecycle moments without needing registry-level observer complexity.
   - Dependency: `LifecycleService`, `SessionManager`, `WorkerSessionConfig`, `ProcessWorkerLauncher`, production server wiring.
   - Evidence: `worker_session.rs` records attach, status updates, worker events, input requests, answer forwarding, spawn failures, deadlines, and child exits; `service.rs` records dispatch and stream start.
   - Failure mode: Some required lifecycle categories are missed, or logs are emitted before failed state changes.
   - Fallback: Move logging one layer closer to `Registry::append_event` and `Registry::update_status` only if evaluator finds missing lifecycle coverage.

9. Hop: Clone-start event text.
   - Assumption: The worker runtime has enough safe repo context to emit `cloning repo: <repo-url>` before Git clone starts.
   - Dependency: `StartJob.repo`, worker runtime `repo_request`, lifecycle repo redaction/identity helpers.
   - Evidence: `run_worker_session_start_job_prepares_repo_before_prompt_runner` shows the runtime receives `StartJob.repo` and sends `repo_preparation` before calling the repo preparer.
   - Failure mode: Stream output cannot show clone activity with a repo URL, or the raw URL leaks credentials.
   - Fallback: Have daemon dispatch append a separate clone-start event with redacted identity before launch, but this is less accurate if worker never attaches.

10. Hop: Safe one-line terminal output.
    - Assumption: Redacting secrets and replacing control characters at render boundaries is enough to prevent terminal line forging.
    - Dependency: Shared lifecycle safe-line function, CLI output renderer, daemon logger, worker event sanitizer.
    - Evidence: Existing CLI and worker tests already check credential/API-key redaction; no current test covers CR/LF or terminal escape control characters.
    - Failure mode: User-derived text inserts fake timestamped lines, escapes terminal output, or leaks secrets in tables/logs/streams.
    - Fallback: Make safe-line rendering mandatory at every terminal write and add unit tests with newline, carriage return, ESC, and credential-bearing fixtures.

11. Hop: Worker list table preserves order.
    - Assumption: CLI can render a table while preserving daemon registry order.
    - Dependency: `Registry::order`, `Registry::list`, `LifecycleService::list`, CLI table formatter.
    - Evidence: `Registry::list` iterates the `order` vector; PRD requires preserving existing listing order.
    - Failure mode: Sorting for table aesthetics changes user-observed order.
    - Fallback: Do not sort in CLI. Only compute column widths from the already-ordered vector.

12. Hop: Fast event burst readability.
    - Assumption: Existing streaming and event-log capacity are enough when each event renders as a single line.
    - Dependency: `EventLog` bounded retention, `EventSubscription`, gRPC streaming channel capacity, CLI writer loop.
    - Evidence: `Registry` uses bounded per-worker retained events and subscribers; `server.rs` streams replay then live events until terminal events.
    - Failure mode: Output becomes nested/noisy, or truncation is hard to understand.
    - Fallback: Keep the history-truncated item as a single timestamped line and do not add per-event metadata columns in stream output.

## Technical alternatives and tournament

Considered alternatives:

- Shared lifecycle formatter versus duplicated CLI/daemon formatting.
  - Selected: shared lifecycle formatter for timestamp and safe message, with CLI retaining any styling/table details.
  - Reason: two concrete consumers exist and PRD requires consistency.
- Event-time timestamps versus render-time-only timestamps.
  - Selected: populate existing proto timestamp fields from daemon event mutation time, with render-time fallback.
  - Reason: replayed stream events should keep meaningful timestamps without changing proto.
- Daemon logger at service/session boundaries versus registry observer.
  - Selected: service/session logging first.
  - Reason: it covers required lifecycle moments with lower coupling and only logs after successful state changes.
- UUID crate versus manual UUIDv6 implementation.
  - Selected: `uuid` crate if local Cargo resolution supports the documented v6 feature set.
  - Reason: avoids custom ID algorithms and keeps the generator behind the existing daemon seam.
- Strict global `WorkerId` parser versus creation-boundary UUIDv6 generation.
  - Selected: creation-boundary UUIDv6 generation for this feature.
  - Reason: the PRD targets newly-created workers, the registry is in-memory, and broad parser tightening adds test/migration churn without improving worker creation safety if generation is already strict.

Technical tournament recommendation: Not required. The alternatives are implementation placement choices inside the existing architecture, not competing stacks, data models, API families, or rollout strategies. The selected path has the clearest compatibility with current package boundaries and the lowest migration risk.

## Security and privacy

- Validate repo input at CLI and daemon boundaries before worker creation.
- Reject local path-like inputs and local `file://` inputs without echoing unnecessary local filesystem details.
- Redact scheme URL userinfo and provider/API-key-like secrets before daemon logs, CLI lifecycle lines, list tables, stream output, and failure text.
- Treat raw repo URLs as sensitive operational input. Store/display only redacted identities outside clone execution.
- Keep worker one-time token redaction in worker runtime because token knowledge is local to the worker session.
- Replace line breaks, carriage returns, ESC, and other terminal control characters in user-derived messages so one source event cannot visually forge multiple lifecycle lines.
- Do not add authorization, RBAC, durable audit logs, or centralized compliance reporting; those are PRD non-goals.

## Performance and operations

- Lifecycle line rendering is O(message length) and should be negligible relative to daemon RPC and worker execution.
- CLI table width computation should be O(worker count * displayed field length) and should not sort rows.
- Event streaming remains append-only line output; no dynamic redraw, spinner, JSON decoding, or nested rendering.
- Daemon terminal logging writes one line per lifecycle-relevant moment only. It must not log verbose protocol payloads or every low-level frame if that frame does not advance lifecycle understanding.
- Bounded event retention remains in `Registry` through `event_capacity`; truncation remains explicit and line-oriented.
- Production daemon logging to stderr keeps stdout available for future machine-readable daemon startup output if needed.
- UUIDv6 generation should be thread-safe if dispatch can be called concurrently. The production generator must share any required UUID clock/context across calls.

## Testing strategy

Use existing package-level `tests/` directories and Red-Green-Refactor.

Lifecycle tests:

- `WorkerId` generation returns UUIDv6 values for production generator helper or lifecycle identity constructor.
- Repo URL validation accepts representative `https`, `ssh`, `git`, and SCP-like remote URLs.
- Repo URL validation rejects relative paths, absolute POSIX paths, Windows paths, UNC paths, blank strings, and `file://` inputs.
- Repo identity redacts credentials and keeps enough host/path context.
- Terminal line formatter renders `[<timestamp>] <message>`.
- Safe-line rendering removes CR/LF/ESC/control characters and redacts API-key-like secrets.

CLI tests:

- `feature` valid URL prints connecting, connected, creating worker, and created worker lines in order.
- `feature` omits old `[accepted] Lifecycle worker`, `Mode`, `Repo`, `Status`, and `Scope` accepted-worker detail.
- `feature` invalid path-like repo rejects before client connection/dispatch.
- `list` prints connection lines, headers, status, worker id, git repo, and preserves order.
- empty list still prints connection lines and a readable table/empty-state representation with required headers.
- `worker` stream prints `started` first after connection and then one timestamped line per event.
- stream clone activity renders `cloning repo: <repo-url>` with corrected spelling.
- stream/control-character fixtures cannot produce forged additional lifecycle lines.

Daemon tests:

- dispatch rejects path-like repos before ID allocation, registry insertion, layout creation, or launch.
- production or test UUIDv6 generator returns UUIDv6 and dispatch exposes that ID.
- dispatch records redacted repo identity and never stores credential-bearing identity for display.
- service/list responses preserve registry order and include timestamp fields where available.
- logger records worker creation, stream start, worker attach, status update, input requested, answer provided, failure, success, stop, and spawn/attach deadline failure lines.
- logger tests use recording/noop logger and do not assert against real terminal output.

Worker tests:

- runtime emits `repo_preparation` as `cloning repo: <redacted-repo-url>` before repo preparer execution.
- repo preparation failures still redact credentials and secrets.
- worker session success/failure still sends exactly one terminal status.

Integration/regression tests:

- lifecycle service replay preserves milestone names and includes occurred-at timestamps.
- fast event replay/tail remains ordered by sequence and line-oriented.
- existing answer-input validation and worker terminal behavior continue to pass.

Recommended commands for implementation validation:

- `cargo test -p lifecycle`
- `cargo test -p cli lifecycle_output`
- `cargo test -p daemon`
- `cargo test -p worker runtime repo`

## Rollout and migration

- Implement in focused efforts:
  1. shared lifecycle repo validation, redaction, safe-line formatting, and UUIDv6 generation support;
  2. daemon dispatch validation, UUIDv6 production generator, timestamps, and logging boundary;
  3. CLI connection-aware output, feature/list/stream renderers, and output tests;
  4. worker clone-start event wording and regression tests.
- No data migration is needed because lifecycle registry state is in-memory.
- Update `Cargo.toml`/`Cargo.lock` only in implementation efforts that add `uuid`, `url`, or timestamp dependencies.
- Update `docs/architecture-and-packages.md` during implementation only if package responsibility wording becomes stale. The current document already lists lifecycle identity/repo/event contracts and daemon lifecycle logging ownership is an additive implementation detail.
- Keep `STATE.md` unchanged until evaluator/coordinator review. Decomposition must wait for staged TDD evaluator acceptance and user approval.

## Risks

- CLI connection refactor risk: If connection is not exposed cleanly, lifecycle lines may appear in the wrong order. Mitigation: test with a fake client that records connection and RPC order.
- Repo syntax risk: Git remote syntax is broader than standard URL parsing. Mitigation: explicitly test allowed cloneable forms and rejected local forms; start with a bounded accepted syntax.
- UUID dependency risk: The selected crate feature set could fail local Cargo resolution or require RNG configuration. Mitigation: keep generation behind the existing `IdGenerator` seam and verify dependency resolution before broad code changes.
- Parser compatibility risk: Tightening `WorkerId::from_str` globally could break fixtures and any retained string references. Mitigation: generate UUIDv6 at creation first and let evaluators decide whether strict parsing is required.
- Daemon log duplication risk: Logging both status updates and derived events can duplicate messages. Mitigation: emit only lifecycle-relevant lines after successful state changes and test representative flows.
- Secret leakage risk: Raw repo URLs and event text cross multiple surfaces. Mitigation: use shared safe-line rendering for every terminal write and keep raw repo only at clone boundary.
- Timestamp consistency risk: Some test/legacy proto events may have no `occurred_at`. Mitigation: CLI fallback to render time, with tests for both present and absent timestamps.
- Scope creep risk: Safety requirements could expand into durable audit or structured logging. Mitigation: keep changes bounded to terminal output and validation behavior.

## Open questions

None blocking.

Non-blocking evaluator checks:

- Confirm whether global `WorkerId::from_str` should reject non-UUIDv6 values, or whether UUIDv6 generation at creation is sufficient for this PRD's newly-created-worker scope.
- Confirm the final accepted cloneable URL syntax, especially whether SCP-like `git@host:path` must be accepted in the first implementation effort.
- Confirm whether daemon terminal lifecycle logs should write to stderr as recommended or stdout if product reviewers expect stdout specifically.
