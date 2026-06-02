# Product Requirements Document

## Problem statement

Doric daemon and CLI lifecycle activity is too difficult to follow during feature runs because users and operators must infer progress from verbose worker-acceptance details, sparse daemon terminal visibility, or event payloads that are not consistently rendered as concise terminal lines. The product needs readable, timestamped lifecycle output for daemon, agent, worker, and event-stream moments while preserving existing worker lifecycle semantics.

The final champion optimizes for predictable, low-overhead, line-oriented terminal output under normal and fast event rates. It also adds bounded safety requirements so lifecycle logs remain useful without exposing credentials, leaking unnecessary local filesystem details, or allowing user-derived content to visually forge extra terminal lifecycle lines.

## Goals

- Make daemon and CLI lifecycle activity understandable through concise `[<timestamp>] <message>` terminal lines.
- Replace the `feature` command's verbose accepted-worker detail with the approved connection and worker-creation lifecycle sequence.
- Show daemon connection lifecycle lines and a simple worker table when listing workers or agents.
- Render CLI event streams as timestamped line-oriented messages that begin with `started` and remain readable during fast event bursts.
- Ensure worker IDs shown in lifecycle output, daemon logs, event output, serialized worker references, and worker tables are UUIDv6 values for newly created workers in this feature scope.
- Accept cloneable repository URL syntax only for worker creation, and reject local relative paths, local absolute paths, path-like values, and local `file://` inputs before worker creation.
- Preserve existing worker lifecycle behavior except where lifecycle logging, UUIDv6 ID generation, output rendering, or URL-only repository validation must change.
- Avoid additional sensitive data exposure by redacting credentials and rendering user-derived lifecycle content as safe one-line terminal output.

## Non-goals

- Do not redesign the full daemon logging or tracing system beyond lifecycle terminal logs needed for this feature.
- Do not introduce JSON logs, rich TUI rendering, progress spinners, dynamic redraws, or nested structured logs for the requested lifecycle output.
- Do not change worker lifecycle semantics except for logging, UUIDv6 IDs, output rendering, and URL-only repository validation.
- Do not allow local filesystem paths, including relative paths, absolute paths, or local `file://` URLs, as worker repository inputs.
- Do not add durable audit-log storage, centralized compliance reporting, role-based access-control changes, or encryption features as part of this product scope.
- Do not add new CLI commands or daemon APIs solely for this feature unless existing command or API boundaries cannot support the required user-visible behavior.
- Do not require users to decode structured event payloads to understand lifecycle progress.
- Do not specify technical implementation design, decomposition, tests, or code changes in this PRD.

## Personas

- Feature-run initiator: runs Doric CLI commands such as `feature` and wants short progress output that confirms daemon connection and worker creation without verbose worker internals.
- Daemon operator: watches the daemon terminal and needs lifecycle moments to be visible, ordered, and low-noise during active work.
- Agent workflow maintainer: verifies worker creation, event streaming, and status changes without parsing noisy accepted-worker output.
- Implementation engineer: updates daemon/CLI boundaries, worker ID generation, repository validation, and tests while preserving existing workflow semantics.

## Generator persona debate

The scale/performance candidate won the tournament because it best matched the prompt's readability and lifecycle-NFR needs: fast event bursts, bounded output volume, low-overhead terminal lines, failure/completion visibility, and predictable stream behavior. The accepted tradeoff is that lifecycle output stays intentionally terse and omits low-value metadata that would make terminal streams harder to scan.

The security/compliance candidate was grafted into the champion where it stayed within product scope. Credential redaction, privacy/audit safeguards, safe one-line rendering, terminal-output integrity, and local-data exposure controls improve daemon and CLI lifecycle output without requiring a full audit system or structured logging redesign.

The lean-UX candidate remains a low-friction influence only. The final champion keeps existing command flows, concise connection and creation messages, clear validation failures, simple tables, and no guided wizard or extra workflow surface.

Accepted tradeoffs:

- Plain append-only terminal lines are preferred over richer visual affordances because they are more predictable under fast event rates and match the prompt constraints.
- URL-only repository input may block users who previously relied on local paths, but it is an explicit resolved decision and narrows ambiguous worker creation behavior.
- Redaction and safe one-line rendering add modest product complexity, but they protect privacy and terminal-output integrity across daemon logs, CLI output, listing tables, and event streams.
- Failure and completion lifecycle lines are included because they improve operator trust, but output remains limited to lifecycle-relevant messages rather than verbose protocol payloads.

## Primary workflows

1. Feature-run lifecycle output:
   - The user runs the existing `feature` CLI command with a valid repository URL.
   - The CLI prints `[<timestamp>] connecting to daemon...`.
   - After connection succeeds, the CLI prints `[<timestamp>] connected to daemon...`.
   - The CLI prints `[<timestamp>] creating worker...`.
   - After worker creation succeeds, the CLI prints `[<timestamp>] created worker: <worker-id>`.
   - The displayed worker ID is a UUIDv6 value.
   - The CLI does not print the previous verbose accepted-worker detail in this flow.

2. Worker/agent listing output:
   - The user runs the existing worker/agent listing command.
   - The CLI prints `[<timestamp>] connecting to daemon...`.
   - After connection succeeds, the CLI prints `[<timestamp>] connected to daemon...`.
   - The CLI prints a simple table with clear headers for worker status, worker id, and git repo.
   - The listing preserves the existing listing order.
   - Worker IDs in the table are UUIDv6 values.

3. Event stream output:
   - The user starts the existing CLI event stream.
   - The stream begins with `[<timestamp>] started`.
   - Repository clone activity appears as a timestamped line such as `[<timestamp>] cloning repo: https://foo.bar`.
   - Subsequent daemon, agent, worker, user-message, status, failure, completion, and comparable lifecycle events appear as one timestamped string message per terminal line.
   - The stream does not add separate connection lifecycle lines unless the existing stream command flow already requires connection lifecycle lines.

4. Daemon operator terminal visibility:
   - The daemon terminal emits timestamped string messages for user-message activity, agent status updates, worker creation, repository clone start, stream start, relevant failures, completion, and comparable daemon/agent lifecycle moments.
   - Each emitted lifecycle message is concise enough to remain readable when events arrive quickly.
   - Terminal output remains line-oriented and does not require structured-log decoding.

5. Invalid repository input:
   - A user or non-CLI caller attempts worker creation with a relative path, absolute path, path-like value, or local `file://` value.
   - The request is rejected before worker creation.
   - The CLI-facing failure message is understandable and communicates that worker repositories must be URLs.
   - No worker ID is allocated or displayed for the rejected input.

6. Safe lifecycle rendering:
   - A repository URL or lifecycle message contains embedded credentials, tokens, passwords, control characters, line breaks, or local filesystem details.
   - Terminal output preserves enough non-secret context for progress visibility and support.
   - Secret material is redacted, local path details from rejected inputs are not unnecessarily echoed, and each source event renders as one safe displayed line.

## Functional requirements

- FR1: The `feature` CLI command must print the following lifecycle sequence for successful worker creation: `[<timestamp>] connecting to daemon...`, `[<timestamp>] connected to daemon...`, `[<timestamp>] creating worker...`, and `[<timestamp>] created worker: <worker-id>`.
- FR2: The successful `feature` command flow must stop printing verbose accepted-worker detail.
- FR3: CLI and daemon lifecycle lines must use a straightforward timestamp plus string-message format.
- FR4: Timestamp rendering must use the repository's existing timestamp style when one exists; otherwise it must use a compact ISO-8601/RFC3339-style timestamp.
- FR5: Worker/agent listing must print connection lifecycle lines before the table.
- FR6: Worker/agent listing must render a simple table with clear headers for worker status, worker id, and git repo.
- FR7: Worker/agent listing must preserve the existing listing order.
- FR8: CLI event streaming must render each lifecycle/event message as a timestamped terminal line.
- FR9: CLI event streaming must begin with `[<timestamp>] started`.
- FR10: CLI event streaming must render repository clone activity with corrected spelling as `cloning repo: <repo-url>`.
- FR11: Daemon terminal logs must include user-message activity, agent status updates, worker creation, repository clone start, stream start, relevant failures, completion, and comparable daemon/agent lifecycle moments.
- FR12: Lifecycle output must remain line-oriented under fast event rates; each source event must occupy one complete displayed terminal line and must not require multi-line nesting to understand normal progress.
- FR13: Lifecycle messages must be concise and bounded to information needed for operator progress visibility.
- FR14: Worker IDs created for this feature flow must be UUIDv6 values.
- FR15: UUIDv6 worker IDs must be used consistently in CLI lifecycle output, daemon logs, worker tables, serialized worker references, and displayed event output.
- FR16: Worker repository inputs must accept cloneable URL syntax only.
- FR17: Local relative paths, local absolute paths, path-like values, and local `file://` inputs must be rejected for worker creation.
- FR18: URL-only repository validation must protect both CLI argument handling and daemon worker-creation APIs.
- FR19: A path-like repository validation failure must be understandable to a CLI user and must not create a worker.
- FR20: Existing worker lifecycle behavior must be preserved except where logging, ID generation, output rendering, or repository validation changes are required by this PRD.
- FR21: Repository URLs and user-derived lifecycle messages displayed in daemon logs, CLI lifecycle output, listing tables, or event streams must not expose embedded credentials, API keys, tokens, passwords, or comparable secret material.
- FR22: Rejected local repository inputs must not echo unnecessary local filesystem details in user-facing output.
- FR23: User-derived lifecycle message content containing line breaks or terminal control characters must not be able to visually forge additional lifecycle lines.
- FR24: Privacy and audit safeguards must remain bounded to terminal lifecycle output and validation behavior; they must not require durable audit storage, centralized reporting, or access-control changes.

## User Value Hops

1. Hop 1: Connection visibility.
   - User action: A feature-run initiator starts the `feature` command.
   - Expected product response: The CLI emits `[<timestamp>] connecting to daemon...` and later `[<timestamp>] connected to daemon...` after connection succeeds.
   - Testable evidence: A CLI run against an available daemon shows both lines in sequence with valid timestamps and no verbose accepted-worker detail before connection confirmation.

2. Hop 2: Worker creation visibility.
   - User action: The same valid `feature` command proceeds to worker creation.
   - Expected product response: The CLI emits `[<timestamp>] creating worker...` and then `[<timestamp>] created worker: <worker-id>`.
   - Testable evidence: The created-worker line includes one UUIDv6 worker ID and omits verbose accepted-worker internals.

3. Hop 3: URL-only input protection.
   - User action: A user tries to create a worker with a relative path, absolute path, path-like value, or local `file://` input.
   - Expected product response: The CLI or daemon rejects the request with an understandable URL-only message and no worker is created.
   - Testable evidence: Rejected inputs produce no created-worker line, no worker table entry, and no allocated worker ID.

4. Hop 4: Operator lifecycle visibility.
   - User action: A daemon operator watches the daemon terminal during worker creation, user-message activity, agent status updates, repository clone start, stream start, failures, and completion.
   - Expected product response: The daemon emits concise timestamped string messages for those lifecycle moments.
   - Testable evidence: A representative feature run produces one readable timestamped line per required lifecycle category.

5. Hop 5: Worker list scanability.
   - User action: An agent workflow maintainer runs the worker/agent listing command.
   - Expected product response: The CLI prints connection lifecycle lines followed by a simple table with worker status, worker id, and git repo in the existing listing order.
   - Testable evidence: The table has clear headers, includes UUIDv6 worker IDs, includes the git repo value, keeps the known existing ordering, and handles empty listings clearly.

6. Hop 6: Stream startup clarity.
   - User action: A user starts the CLI event stream.
   - Expected product response: The stream begins with `[<timestamp>] started` and does not add separate connecting lines unless the existing stream command flow already requires them.
   - Testable evidence: Stream startup output contains the started line as the first lifecycle event line and follows the resolved default for connection-line behavior.

7. Hop 7: Fast event readability.
   - User action: Daemon, agent, and worker events arrive quickly while a CLI stream is active.
   - Expected product response: Each event is rendered as a complete timestamped string line, including clone activity as `cloning repo: <repo-url>` and subsequent action/event strings.
   - Testable evidence: A fast burst of lifecycle events remains line-oriented, ordered according to the stream's observable event order, and free of nested structured-log formatting.

8. Hop 8: Safe terminal output.
   - User action: A repository URL or lifecycle message contains credentials, tokens, passwords, control characters, line breaks, or local filesystem details from a rejected path-like input.
   - Expected product response: Terminal output remains useful without exposing secrets, unnecessarily disclosing local paths, or visually forging extra lifecycle lines.
   - Testable evidence: Secret-bearing inputs are redacted on all lifecycle surfaces, rejected path-like input errors do not reveal unnecessary local details, and each source event renders as one safe displayed line.

9. Hop 9: Low-overhead lifecycle completion.
   - User action: The workflow reaches relevant failure or completion moments.
   - Expected product response: The daemon and CLI expose concise timestamped failure/completion lifecycle messages without changing worker lifecycle semantics.
   - Testable evidence: Success and failure paths produce lifecycle lines that are readable, bounded, and do not require implementation-specific protocol details.

## Acceptance criteria

- Success: A successful `feature` command with a valid repository URL prints the required connection and worker-creation lifecycle lines in order and includes a UUIDv6 worker ID in `created worker: <worker-id>`.
- Success: The successful `feature` command no longer prints verbose accepted-worker details.
- Success: A worker/agent listing command prints connection lifecycle lines followed by a simple table with clear headers for worker status, worker id, and git repo.
- Success: Worker listing preserves the existing listing order.
- Success: CLI event streaming begins with `[<timestamp>] started`.
- Success: CLI event streaming renders clone activity as `[<timestamp>] cloning repo: <repo-url>`.
- Success: CLI event streaming renders each subsequent event as one timestamped string line.
- Success: Daemon terminal output includes timestamped lifecycle messages for user-message activity, agent status updates, worker creation, repository clone start, stream start, relevant failures, completion, and comparable daemon/agent lifecycle moments.
- Success: Worker IDs visible in CLI output, daemon logs, event stream output, serialized worker references, and worker tables are UUIDv6 values for newly created workers in this feature scope.
- Failure: Relative local paths are rejected for worker creation with an understandable URL-only message and no worker is created.
- Failure: Absolute local paths are rejected for worker creation with an understandable URL-only message and no worker is created.
- Failure: Local `file://` inputs are rejected for worker creation with an understandable URL-only message and no worker is created.
- Failure: Daemon-side worker creation rejects non-URL repository inputs even when the caller is not the CLI.
- Empty state: Worker/agent listing with no workers still prints connection lifecycle lines and a readable empty table or established empty-list representation with the required headers.
- Empty state: Event streaming can start and emit `[<timestamp>] started` even before later lifecycle events arrive.
- Edge state: Fast event bursts remain line-oriented, with one complete terminal line per source event and no nested structured-log output.
- Edge state: If the existing stream command flow already requires connection lifecycle lines, those lines may appear before stream events; otherwise stream output starts with `[<timestamp>] started`.
- Edge state: Lifecycle logging remains low-noise by avoiding verbose protocol payloads, accepted-worker internals, or rich TUI redraw behavior.
- Privacy: Repository URLs and user-derived messages containing credentials, tokens, API keys, or passwords do not display those secrets in daemon logs, CLI lifecycle output, listing tables, or event streams.
- Privacy: Rejected local path-like inputs are not echoed in a way that discloses unnecessary local filesystem details.
- Integrity: User-derived event text containing line breaks or terminal control characters cannot visually forge additional lifecycle lines.
- Regression guard: Existing worker lifecycle semantics are unchanged except for logging, UUIDv6 IDs, output rendering, and URL-only repository validation.
- Scope guard: The feature does not introduce JSON logs, rich TUI rendering, nested structured logs, durable audit storage, centralized compliance reporting, access-control changes, or local-path repository support.

## Product alternatives and tournament

The PRD tournament evaluated three unique candidates:

- `scale_v1`: scale/performance advocate focused on fast-event readability, bounded output volume, low-overhead lifecycle lines, failure/completion visibility, stream startup behavior, UUIDv6 identity, and URL-only validation.
- `security_v1`: security/compliance advocate focused on credential redaction, privacy/audit safeguards, terminal-output integrity, safe one-line rendering, and local-data exposure controls.
- `lean_v1`: lean-UX advocate focused on low-friction command flows, minimal user input, clear validation failures, simple tables, and existing integration patterns.

Final Elo ranking:

- Rank 1: `scale_v1`, Elo `1031.233`, record `2-0`.
- Rank 2: `security_v1`, Elo `999.264`, record `1-1`.
- Rank 3: `lean_v1`, Elo `969.503`, record `0-2`.

`scale_v1` won because it best satisfies the prompt's core NFRs for readable, bounded, line-oriented lifecycle output under fast event rates while preserving the simple terminal model. `security_v1` was the strongest graft source because its safety requirements improve privacy and terminal integrity without contradicting the prompt. `lean_v1` influenced the final champion only through compatible low-friction choices: existing command flows, concise output, simple tables, understandable validation messages, and no extra wizard or rich interface.

## Evolution summary

Champion version: Current Champion v1.

Base candidate: `scale_v1`.

Grafted compatible concepts from `security_v1`:

- Credential redaction for repository URLs and user-derived lifecycle messages before display on daemon logs, CLI lifecycle output, listing tables, or event streams.
- Privacy and audit safeguards bounded to terminal lifecycle output, failure/completion visibility, and URL-only validation.
- Safe one-line terminal rendering for user-derived messages containing line breaks or terminal control characters.
- Terminal-output integrity requirements that prevent source events from visually forging additional lifecycle lines.
- Local-data exposure controls for rejected path-like inputs and local `file://` inputs.

Compatible low-friction influence from `lean_v1`:

- Keep existing command flows.
- Keep lifecycle messages concise.
- Keep worker listing as a simple table.
- Keep validation failures understandable.
- Avoid new commands, wizards, rich TUI behavior, or extra user steps.

Rejected concepts:

- Full structured tracing, JSON logs, nested log formats, rich TUI rendering, progress spinners, and terminal redraw behavior because they contradict the prompt's plain timestamp-plus-message constraint.
- Durable audit-log storage, centralized compliance reporting, access-control changes, and encryption features because they exceed the requested product scope.
- Local path or local `file://` repository support because the resolved decision requires URL-only worker repository input.
- Echoing raw credential-bearing URLs, raw local paths from rejected inputs, or unescaped multiline/control-character content because those undermine privacy and terminal-output integrity.
- A narrower lean-only scope that changes only the `feature` command and listing output while leaving daemon lifecycle coverage minimal, because the prompt explicitly requires daemon logs for user-message activity, agent status updates, worker creation, clone start, stream start, failures, completion, and comparable moments.

PRD sections changed by evolution: problem statement, goals, non-goals, generator persona debate, primary workflows, functional requirements, User Value Hops, acceptance criteria, product alternatives and tournament, evolution summary, success measures, and risks and compliance.

## Success measures

- A valid `feature` command produces the expected connection and worker-creation lifecycle sequence with a UUIDv6 worker ID and without verbose accepted-worker detail.
- Worker/agent listing output includes connection lifecycle lines and a readable table with worker status, worker id, and git repo.
- CLI event stream output begins with `[<timestamp>] started` and renders each subsequent event as one timestamped line.
- Representative daemon lifecycle scenarios produce terminal-visible timestamped messages for all mandatory lifecycle categories.
- Path-like repository inputs are rejected before worker creation in both CLI and daemon/API entry points.
- Tested credential-bearing fixture values are not exposed in daemon logs, CLI lifecycle output, listing tables, or event streams.
- Tested user-derived messages with line breaks or terminal control characters render as one safe displayed lifecycle line.
- Fast event burst scenarios remain ordered, line-oriented, and free of nested structured-log formatting.
- Success, failure, empty, edge, privacy, and regression acceptance criteria are covered by validation scenarios before implementation is considered complete.

## Risks and compliance

- Fast-event noise risk: Emitting too many low-value lifecycle lines could make output hard to scan. Product constraint: emit only lifecycle-relevant messages and keep messages concise and bounded.
- Backward-compatibility risk: Users who relied on local paths will be blocked. Product constraint: errors must clearly explain URL-only repository input and must not create a worker.
- UUID consistency risk: Some surfaces could continue showing old worker ID formats. Product constraint: UUIDv6 must be used for newly created worker IDs across CLI lifecycle output, daemon logs, worker tables, serialized worker references, and displayed event output.
- Spelling regression risk: The original example used `clonning repo`. Product constraint: event-stream repository clone output must use `cloning repo`.
- Credential leakage risk: Repository URLs or messages may contain secrets. Product constraint: lifecycle surfaces must redact credentials, API keys, tokens, passwords, and comparable secret material.
- Local data exposure risk: Rejected local path inputs could reveal local machine details. Product constraint: validation failures must be understandable without unnecessarily echoing local filesystem paths.
- Terminal-output integrity risk: User-derived line breaks or control characters could visually forge additional lifecycle lines. Product constraint: each source event must render as one safe displayed line.
- Scope creep risk: Safety requirements could expand into a full compliance system. Product constraint: privacy/audit safeguards stay bounded to terminal lifecycle output and validation behavior.

## Open questions

None blocking. The prompt records that all open questions were resolved on 2026-06-02 and the user approved the proposed defaults.
