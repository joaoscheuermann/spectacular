# Features

## Source artifacts

- `.doric/06-2026/02/20260602-165401-daemon_cli_lifecycle_logs/PROMPT.md`
- `.doric/06-2026/02/20260602-165401-daemon_cli_lifecycle_logs/PRD.md`
- `.doric/06-2026/02/20260602-165401-daemon_cli_lifecycle_logs/TDD.md`
- `.agents/skills/doric/references/04-decomposition.md`

This extraction is scoped to the accepted PRD and TDD only. It does not add implementation work outside lifecycle terminal output, UUIDv6 worker identity generation for newly created workers, URL-only repository validation, safe terminal rendering, daemon lifecycle logging, CLI lifecycle rendering, event stream rendering, worker clone-start wording, and focused regression coverage.

## Extracted features

### F1: Shared timestamped lifecycle line formatting and safe terminal rendering

Create a shared lifecycle terminal rendering layer for plain timestamp-plus-message lines.

- Source trace: PRD FR3, FR4, FR8, FR12, FR13, FR21, FR23, FR24; TDD proposed design 1; Dependency Hops 1 and 10.
- Product behavior: Daemon logs, CLI lifecycle lines, worker tables, and event streams render readable `[<timestamp>] <message>` output where line content is concise, one-line, and terminal-safe.
- Technical behavior: `lifecycle` owns timestamp formatting, safe message rendering, credential redaction helpers, and repo display helpers where shared use is practical.
- Constraints: Keep formatter plain text; keep CLI styling/table choices in `cli`; do not introduce JSON logs, rich TUI output, progress spinners, or nested structured logs.

### F2: URL-only clone repository validation and redacted repository identity

Accept cloneable repository URL syntax only, reject local/path-like inputs before worker creation, and keep raw repository input scoped to clone execution.

- Source trace: PRD FR16, FR17, FR18, FR19, FR21, FR22, FR24; TDD proposed design 2; Dependency Hops 3 and 4.
- Product behavior: CLI users and non-CLI daemon callers receive understandable URL-only failures for relative paths, absolute paths, path-like values, UNC/Windows paths, and local `file://` inputs; rejected inputs do not create workers or unnecessarily echo local filesystem details.
- Technical behavior: Shared lifecycle validation defines accepted cloneable remote syntax, including explicit scheme URLs and the accepted SCP-like Git remote syntax decision. CLI validates before connecting; daemon validates before ID allocation, layout creation, registry insertion, accepted-event append, or worker launch.
- Constraints: Raw repo URL is available only for `git clone`; redacted identity is used for registry summaries, daemon logs, CLI tables, and stream messages.

### F3: UUIDv6 worker identity generation with compatibility guardrails

Generate UUIDv6 worker IDs for newly created production workers while preserving typed `WorkerId` compatibility unless broad parser tightening becomes necessary.

- Source trace: PRD FR14 and FR15; TDD proposed design 3 and data model; Dependency Hops 5 and 6.
- Product behavior: Worker IDs shown in CLI lifecycle output, daemon logs, worker tables, serialized worker references, and displayed event output are UUIDv6 values for newly created workers in this feature scope.
- Technical behavior: Production worker ID generation stays behind the existing daemon `IdGenerator` boundary, with deterministic test injection retained. The UUIDv6 node-id strategy is encapsulated behind `IdGenerator` rather than spread across callers.
- Constraints: Do not globally tighten `WorkerId::from_str` unless evaluator or implementation evidence shows it is needed. Descriptive non-production IDs may remain in tests that do not exercise newly-created-worker requirements.

### F4: Daemon lifecycle timestamps and injected terminal logger

Populate daemon lifecycle timestamps and emit concise daemon terminal lifecycle logs through an injected logging boundary.

- Source trace: PRD FR11, FR12, FR13, FR20; TDD proposed design 4 and 5; Dependency Hops 7 and 8.
- Product behavior: Daemon terminal output includes readable timestamped lifecycle messages for user-message activity, agent status updates, worker creation, repository clone start, stream start, relevant failures, completion, and comparable daemon/agent lifecycle moments.
- Technical behavior: Daemon registry records carry update/occurred timestamps for `WorkerSummary.updated_at` and `WorkerEvent.occurred_at` without changing protobuf. A daemon-local `LifecycleLogger` is injected through `LifecycleService` and `SessionManager`, with production writing terminal lifecycle lines and tests using recording/noop loggers.
- Constraints: Emit logs after successful state changes; avoid verbose protocol payloads; prefer stderr for production daemon terminal logging unless implementation review decides otherwise.

### F5: Feature command lifecycle output and explicit CLI connection boundary

Replace verbose accepted-worker output for `feature` with the approved lifecycle sequence.

- Source trace: PRD FR1, FR2, FR3, FR4, FR14, FR15, FR16, FR17, FR18, FR19, FR20, FR21, FR23; TDD proposed design 6; Dependency Hop 2.
- Product behavior: A valid `feature` command prints `connecting to daemon...`, `connected to daemon...`, `creating worker...`, and `created worker: <worker-id>` in order, with a UUIDv6 worker ID and no old accepted-worker detail.
- Technical behavior: CLI command handling validates prompt/repo first, exposes connection success before dispatch, renders action lifecycle lines from CLI command code, and surfaces daemon validation failures through safe/redacted error output.
- Constraints: Keep `debug` outside the required output change unless implementation intentionally reuses the new lifecycle renderer; any `debug` output change must be called out and tested as intentional.

### F6: Worker/agent listing connection lines and simple table

Render worker/agent listing output as connection lifecycle lines followed by a simple ordered table.

- Source trace: PRD FR5, FR6, FR7, FR14, FR15, FR20, FR21, FR22; TDD proposed design 6 and data/API contracts; Dependency Hop 11.
- Product behavior: Listing prints `connecting to daemon...`, `connected to daemon...`, then a clear table with worker status, worker id, and git repo. Existing listing order is preserved, and empty listings remain readable with required headers or established empty-list representation.
- Technical behavior: CLI computes table widths from daemon-provided worker order without sorting; table repo values are redacted and terminal-safe.
- Constraints: Keep listing low-noise and table-like; do not introduce rich TUI redraws or extra workflow steps.

### F7: CLI event stream lifecycle renderer and worker clone-start wording

Render worker event streams as one timestamped safe line per event and update clone-start wording.

- Source trace: PRD FR8, FR9, FR10, FR11, FR12, FR13, FR20, FR21, FR23; TDD proposed design 6 and 7; Dependency Hops 9 and 12.
- Product behavior: Stream output begins with `[<timestamp>] started`, renders clone activity as `cloning repo: <repo-url>`, and renders subsequent daemon, agent, worker, user-message, status, failure, completion, truncation, and comparable lifecycle events as one complete timestamped line each.
- Technical behavior: CLI uses event `occurred_at` when present and render-time fallback when absent. Worker runtime emits `repo_preparation` as `cloning repo: <redacted-repo-url>` before repo preparer execution.
- Constraints: Do not add separate stream connection lifecycle lines unless the existing stream command flow already requires them. Keep truncation and waiting-for-input output line-oriented.

### F8: Lifecycle semantics preservation and focused regression coverage

Preserve existing worker lifecycle semantics while covering all product and technical changes with focused tests.

- Source trace: PRD FR20 and scope guards; TDD proposed design 8, testing strategy, rollout/migration, and risks.
- Product behavior: Worker lifecycle behavior remains unchanged except for logging, UUIDv6 IDs, output rendering, and URL-only repository validation.
- Technical behavior: Existing registry status transitions, input-answer validation, event capacity behavior, worker launch/session attach protocol, prompt-agent runtime flow, gRPC method names, and durable storage boundaries stay intact. Tests cover lifecycle, CLI, daemon, worker, integration, privacy, edge, failure, empty-state, and regression behavior using existing package-level patterns.
- Constraints: No durable audit-log storage, centralized reporting, RBAC, encryption changes, schema migration, new daemon API, or new CLI command is introduced for this scope.

## Requirement coverage map

### Functional requirements

| Requirement | Coverage |
| --- | --- |
| FR1: `feature` prints connecting, connected, creating worker, created worker | F5 |
| FR2: successful `feature` stops printing verbose accepted-worker detail | F5 |
| FR3: CLI and daemon lifecycle lines use timestamp plus message | F1, F4, F5, F6, F7 |
| FR4: timestamp uses existing style or compact ISO/RFC3339 fallback | F1 |
| FR5: worker/agent listing prints connection lifecycle lines before table | F6 |
| FR6: listing renders table headers for status, worker id, git repo | F6 |
| FR7: listing preserves existing order | F6 |
| FR8: CLI event stream renders each lifecycle/event as timestamped line | F1, F7 |
| FR9: CLI event stream begins with `[<timestamp>] started` | F7 |
| FR10: clone activity renders as `cloning repo: <repo-url>` | F7 |
| FR11: daemon logs cover user messages, agent status, worker creation, clone start, stream start, failures, completion, comparable moments | F4, F7 |
| FR12: output remains line-oriented under fast event rates | F1, F4, F7 |
| FR13: lifecycle messages are concise and bounded | F1, F4, F7 |
| FR14: newly created worker IDs are UUIDv6 | F3, F5 |
| FR15: UUIDv6 IDs are used across CLI output, daemon logs, tables, serialized references, displayed events | F3, F4, F5, F6, F7 |
| FR16: worker repository inputs accept cloneable URL syntax only | F2 |
| FR17: local relative paths, absolute paths, path-like values, and local `file://` inputs are rejected | F2 |
| FR18: URL-only validation protects CLI argument handling and daemon APIs | F2, F5 |
| FR19: path-like validation failure is understandable and creates no worker | F2, F5 |
| FR20: preserve worker lifecycle behavior except required changes | F8, with F2-F7 as approved exceptions |
| FR21: lifecycle surfaces do not expose embedded credentials or comparable secrets | F1, F2, F5, F6, F7 |
| FR22: rejected local inputs do not echo unnecessary local filesystem details | F2, F6 |
| FR23: line breaks/control characters cannot visually forge lifecycle lines | F1, F5, F7 |
| FR24: privacy/audit safeguards remain bounded to terminal output and validation | F1, F2, F8 |

### User Value Hops

| User Value Hop | Coverage |
| --- | --- |
| Hop 1: Connection visibility | F5 |
| Hop 2: Worker creation visibility | F3, F5 |
| Hop 3: URL-only input protection | F2, F5 |
| Hop 4: Operator lifecycle visibility | F4, F7 |
| Hop 5: Worker list scanability | F3, F6 |
| Hop 6: Stream startup clarity | F7 |
| Hop 7: Fast event readability | F1, F4, F7 |
| Hop 8: Safe terminal output | F1, F2, F5, F6, F7 |
| Hop 9: Low-overhead lifecycle completion | F4, F7, F8 |

### Acceptance criteria coverage

The PRD acceptance criteria are covered by the same feature map:

- Success criteria for `feature` lifecycle sequence, UUIDv6 ID, and removal of accepted-worker detail: F3, F5.
- Success criteria for worker/agent listing, table shape, empty listings, and ordering: F6.
- Success criteria for stream startup, clone wording, event lines, fast bursts, and optional existing stream connection behavior: F1, F7.
- Success criteria for daemon terminal lifecycle categories: F4, F7.
- Failure criteria for relative paths, absolute paths, local `file://`, daemon-side rejection, understandable messages, and no worker creation: F2, F5.
- Privacy and integrity criteria for redaction, local data exposure, and control-character safety: F1, F2, F5, F6, F7.
- Regression and scope guards for lifecycle semantics and excluded logging/compliance/UI systems: F8.

## Technical coverage map

### TDD proposed design components

| TDD component | Coverage |
| --- | --- |
| Component 1: shared lifecycle terminal output primitives | F1 |
| Component 2: strict clone repository validation in `lifecycle` | F2 |
| Component 3: UUIDv6 worker IDs at daemon dispatch | F3 |
| Component 4: populate event timestamps in daemon registry path | F4, F7 |
| Component 5: daemon terminal logging through injected logger | F4 |
| Component 6: refactor CLI lifecycle command output | F5, F6, F7 |
| Component 7: update worker repo clone event wording | F7 |
| Component 8: preserve existing worker lifecycle semantics | F8 |

### TDD section and contract coverage

| TDD section or contract | Coverage |
| --- | --- |
| Package ownership summary: `lifecycle`, `daemon`, `cli`, `worker` | F1, F2, F3, F4, F5, F6, F7, F8 |
| Current architecture context and observed implementation surfaces | F1-F8 as source evidence for effort planning |
| Technical persona debate and accepted tradeoffs | F1, F2, F3, F4, F5, F8 |
| Data model: in-memory timestamps, event log, repo views, `WorkerId` | F2, F3, F4, F7 |
| Migration: no persistent migration; fixture updates only where creation is tested | F3, F8 |
| CLI contracts for `feature`, `list`, and `worker` | F2, F5, F6, F7 |
| Daemon/service contracts for dispatch, list, events, repo identity | F2, F3, F4, F6, F7 |
| Internal Rust contracts for client connection, logger, worker event | F4, F5, F7 |
| Security and privacy | F1, F2, F5, F6, F7, F8 |
| Performance and operations | F1, F4, F6, F7, F8 |
| Testing strategy | F1-F8 |
| Rollout and migration effort sequence recommendation | F1-F8; to be converted by effort planner into ordered effort files |
| Risks and mitigations | F1-F8 |
| TDD open questions and non-blocking evaluator checks | Assumption coverage below |

### Dependency Hops

| Dependency Hop | Coverage |
| --- | --- |
| DH1: Shared terminal lifecycle line formatter | F1 |
| DH2: Explicit CLI connection lifecycle | F5 |
| DH3: URL-only repository validation | F2 |
| DH4: Daemon-side enforcement before side effects | F2 |
| DH5: UUIDv6 production worker IDs | F3 |
| DH6: Worker ID path safety | F3, F8 |
| DH7: Event timestamps without proto change | F4, F7 |
| DH8: Daemon terminal logs through injected logger | F4 |
| DH9: Clone-start event text | F7 |
| DH10: Safe one-line terminal output | F1, F2, F5, F6, F7 |
| DH11: Worker list table preserves order | F6 |
| DH12: Fast event burst readability | F1, F4, F7 |

### Test coverage obligations

| Test area from TDD | Feature coverage |
| --- | --- |
| Lifecycle tests for UUIDv6 generation, repo validation, redaction, formatter, safe-line rendering | F1, F2, F3 |
| CLI tests for `feature`, invalid repo pre-connection rejection, `list`, empty list, stream, clone wording, control-character safety | F2, F5, F6, F7 |
| Daemon tests for side-effect-free rejection, UUIDv6 dispatch, redacted identity, order, timestamps, logger categories | F2, F3, F4, F6 |
| Worker tests for clone-start wording, credential redaction, terminal status preservation | F7, F8 |
| Integration/regression tests for replay timestamps, fast event ordering, input validation, terminal behavior | F4, F7, F8 |

## Assumption coverage

| Assumption or guardrail | Coverage decision |
| --- | --- |
| UUIDv6 node-id strategy behind `IdGenerator` | Covered by F3. Production UUIDv6 generation remains encapsulated behind daemon `IdGenerator`; tests retain deterministic generator injection. |
| Explicit SCP-like Git remote handling decision | Covered by F2. The accepted validation scope includes explicit scheme URLs and SCP-like `git@host:path` remote syntax when host and path are present. |
| Raw repo URL scoped only to clone execution | Covered by F2 and F7. Raw input is used only for the clone request; redacted identity is used everywhere else, including clone-start display. |
| `debug` output changes intentional if touched | Covered by F5 and F8. `debug` is not required to change; if implementation reuses lifecycle rendering there, tests and notes must identify that as an intentional change. |
| Shared timestamp formatting direct dependency decision | Covered by F1. Shared formatting is a direct lifecycle dependency because daemon and CLI both consume timestamped line output. |
| Daemon logger injected through `LifecycleService` and `SessionManager` | Covered by F4. Logging is injected at service/session boundaries and tested with recording/noop loggers. |
| Do not globally tighten `WorkerId::from_str` unless needed | Covered by F3. Creation-boundary UUIDv6 generation satisfies PRD scope unless evaluator or implementation evidence requires broader parser strictness. |
| Daemon terminal logs write to stderr unless reviewed otherwise | Covered by F4 as a technical implementation assumption, not a product blocker. |
| Stream connection lines only appear if existing stream flow already requires them | Covered by F7. The required stream lifecycle begins with `started`; optional existing connection behavior remains constrained. |
| Existing timestamp style should be reused if present, otherwise compact ISO/RFC3339 | Covered by F1. The formatter selection is shared and testable. |
| No protobuf change is required because timestamp fields already exist | Covered by F4 and F8. Implementation should populate existing fields and tolerate missing values. |
| Existing listing order is registry order | Covered by F6. CLI must not sort rows while rendering table output. |

## Exclusions

The following accepted PRD/TDD non-goals are explicitly excluded from decomposition scope:

- Redesigning the full daemon logging or tracing system beyond lifecycle terminal logs.
- JSON logs, rich TUI rendering, progress spinners, dynamic redraws, nested structured logs, or machine-readable log format changes.
- Worker lifecycle semantic changes beyond lifecycle logging, UUIDv6 IDs, output rendering, timestamps, clone-start wording, and URL-only repo validation.
- Local repository path support, including relative paths, absolute paths, path-like values, UNC paths, Windows paths, and local `file://` inputs.
- Durable audit-log storage, centralized compliance reporting, RBAC, authorization changes, encryption features, or compliance system expansion.
- New CLI commands or daemon APIs solely for this feature unless existing boundaries cannot support the required behavior.
- Protobuf/IDL changes; existing optional timestamp fields should be populated instead.
- Durable schema migration; current lifecycle registry state is in-memory.
- Global strict `WorkerId::from_str` UUIDv6 validation unless implementation/evaluator evidence proves it is needed.
- Broad architecture documentation updates unless implementation changes make current package responsibility wording stale.
- Source code edits during this requirement extraction phase.

## Validator notes

- Every PRD functional requirement FR1-FR24 maps to at least one extracted feature.
- Every User Value Hop 1-9 maps to at least one extracted feature.
- Every TDD proposed design component 1-8 maps to at least one extracted feature.
- Every Dependency Hop 1-12 maps to at least one extracted feature.
- Non-blocking decomposition guardrails are represented in Assumption coverage and should not block effort planning unless the implementation plan violates them.
- No extracted feature expands scope beyond the accepted PRD/TDD.
- Effort planning should prefer the TDD rollout sequence as the dependency order: shared lifecycle primitives and validation first, daemon identity/timestamps/logging second, CLI output third, worker clone-start wording and regressions fourth. The effort planner may split further if ownership or test isolation requires smaller slices.
- Source code was not edited by this extraction artifact.
