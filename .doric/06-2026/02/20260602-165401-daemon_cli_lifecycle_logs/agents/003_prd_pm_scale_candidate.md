# Product Requirements Document

Candidate id: scale_v1
Persona: PM scale/performance advocate
Self-evaluation score: 9/10

## Problem statement

Doric daemon and CLI lifecycle activity is currently too difficult to follow during feature runs because users and operators must infer progress from noisy worker details, sparse daemon terminal visibility, or event payloads that are not consistently rendered as concise terminal lines. Under fast event rates, output must remain readable and predictable instead of becoming verbose, nested, or dependent on structured-log decoding.

This PRD candidate optimizes for robust line-oriented lifecycle output, predictable daemon/operator behavior, and low-overhead logging. It keeps the product scope narrow: emit short timestamped strings for key lifecycle moments, replace verbose accepted-worker output in the CLI, render worker listings as a simple status table, stream timestamped event messages, require UUIDv6 worker IDs, and reject local path-like repository inputs before worker creation.

## Goals

- Make daemon and CLI lifecycle activity understandable through concise terminal lines.
- Keep all user-visible lifecycle lines in a straightforward `[<timestamp>] <message>` shape where the timestamp uses the repository's existing timestamp style, or a compact ISO-8601/RFC3339-style timestamp if no established style exists.
- Replace the `feature` command's verbose accepted-worker detail with predictable lifecycle lines for daemon connection, worker creation, and created-worker ID visibility.
- Make worker/agent listing easy to scan by showing daemon connection lifecycle lines followed by a simple table with worker status, worker id, and git repo.
- Render CLI event streams as timestamped line-oriented messages that stay readable during fast event bursts.
- Ensure worker IDs shown in CLI output, daemon logs, event output, and worker tables are UUIDv6 values.
- Ensure worker creation accepts cloneable repository URL syntax only and rejects local relative paths, local absolute paths, and local `file://` inputs before a worker is created.
- Preserve existing worker lifecycle semantics except where logging, ID generation, output rendering, or repository validation must change.
- Keep lifecycle logging low overhead by favoring concise human-readable messages and bounded output volume over rich formatting or verbose protocol details.

## Non-goals

- Do not redesign the full daemon logging or tracing system beyond lifecycle terminal logs needed for this feature.
- Do not introduce JSON, rich TUI rendering, or nested structured logs for the requested lifecycle output.
- Do not change worker lifecycle semantics except for logging, UUIDv6 IDs, output rendering, and URL-only repo validation.
- Do not allow local filesystem paths, including relative paths, absolute paths, or file URLs that behave as local paths, as worker repo inputs.
- Do not add new CLI commands or daemon APIs solely for this feature unless existing command/API boundaries cannot support the required user-visible behavior.
- Do not require users to decode structured event payloads to understand lifecycle progress.
- Do not make event streaming depend on terminal-specific rich rendering capabilities.
- Do not generate technical design, decomposition artifacts, tests, or implementation changes in this PRD candidate.

## Personas

- Feature-run initiator: runs Doric CLI commands such as `feature` and wants short progress output that confirms daemon connection and worker creation without verbose worker internals.
- Daemon operator: watches the daemon terminal and needs lifecycle moments to be visible, ordered, and low-noise during active work.
- Agent workflow maintainer: verifies worker creation, event streaming, and status changes without parsing noisy accepted-worker output.
- Implementation engineer: updates daemon/CLI boundaries, worker ID generation, repo validation, and tests while preserving existing workflow semantics.

## Generator persona debate

The scale/performance position prioritizes predictable output behavior under high event rates over maximum detail per event. The accepted tradeoff is that lifecycle lines should be intentionally terse and may omit low-value metadata that would make the terminal stream harder to scan or increase per-event rendering overhead.

The scale/performance position also prioritizes stable line boundaries and append-only terminal output over interactive terminal updates. The accepted tradeoff is that the product will not use rich TUI affordances, progress spinners, nested grouped logs, or dynamic redrawing for this feature, even though those could be visually polished in slower flows.

Security and compliance concerns are represented by URL-only repository validation and explicit rejection of local path-like inputs, including local `file://` inputs. The accepted tradeoff is that users who previously relied on local paths must switch to cloneable repository URLs.

Lean UX concerns are represented by minimal connection/creation lines, a simple worker table, and understandable validation failure text. The accepted tradeoff is that the CLI remains plain terminal text instead of adding a guided wizard or a new command flow.

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
   - The daemon terminal emits timestamped string messages for user-message activity, agent status updates, worker creation, repo clone start, stream start, relevant failures, completion, and comparable daemon/agent lifecycle moments.
   - Each emitted lifecycle message is concise enough to remain readable when events arrive quickly.
   - Terminal output remains line-oriented and does not require structured-log decoding.

5. Invalid repository input:
   - A user or non-CLI caller attempts worker creation with a relative path, absolute path, or local `file://` value.
   - The request is rejected before worker creation.
   - The CLI-facing failure message is understandable and communicates that worker repositories must be URLs.
   - No worker ID is allocated or displayed for the rejected input.

## Functional requirements

- FR1: The `feature` CLI command must print the following lifecycle sequence for successful worker creation: `[<timestamp>] connecting to daemon...`, `[<timestamp>] connected to daemon...`, `[<timestamp>] creating worker...`, and `[<timestamp>] created worker: <worker-id>`.
- FR2: The `feature` CLI command must stop printing verbose accepted-worker detail in the successful worker creation flow.
- FR3: CLI and daemon lifecycle lines must use a straightforward timestamp plus string-message format.
- FR4: Timestamp rendering must use the repository's existing timestamp style when one exists; otherwise it must use a compact ISO-8601/RFC3339-style timestamp.
- FR5: Worker/agent listing must print connection lifecycle lines before the table.
- FR6: Worker/agent listing must render a simple table with clear headers for worker status, worker id, and git repo.
- FR7: Worker/agent listing must preserve the existing listing order.
- FR8: CLI event streaming must render each lifecycle/event message as a timestamped terminal line.
- FR9: CLI event streaming must begin with `[<timestamp>] started`.
- FR10: CLI event streaming must render repository clone activity with corrected spelling as `cloning repo: <repo-url>`.
- FR11: Daemon terminal logs must include user-message activity, agent status updates, worker creation, repo clone start, stream start, relevant failures, completion, and comparable daemon/agent lifecycle moments.
- FR12: Lifecycle output must remain line-oriented under fast event rates; each event message must occupy a complete terminal line and must not require multi-line nesting to understand normal progress.
- FR13: Lifecycle messages should be concise and bounded to the information needed for operator progress visibility.
- FR14: Worker IDs created for this feature flow must be UUIDv6 values.
- FR15: UUIDv6 worker IDs must be used consistently in CLI lifecycle output, daemon logs, worker tables, serialized worker references, and displayed event output.
- FR16: Worker repository inputs must accept cloneable URL syntax only.
- FR17: Local relative paths, local absolute paths, and local `file://` inputs must be rejected for worker creation.
- FR18: URL-only repository validation must protect both CLI argument handling and daemon worker-creation APIs.
- FR19: A path-like repository validation failure must be understandable to a CLI user and must not create a worker.
- FR20: Existing worker lifecycle behavior must be preserved except where logging, ID generation, output rendering, or repo validation changes are required by this PRD.

## User Value Hops

1. Hop 1: Connection visibility
   - User action: A feature-run initiator starts the `feature` command.
   - Expected product response: The CLI immediately emits `[<timestamp>] connecting to daemon...` and later `[<timestamp>] connected to daemon...` after connection succeeds.
   - Testable evidence: A CLI run against an available daemon shows both lines in sequence with valid timestamps and no verbose accepted-worker detail before connection confirmation.

2. Hop 2: Worker creation visibility
   - User action: The same valid `feature` command proceeds to worker creation.
   - Expected product response: The CLI emits `[<timestamp>] creating worker...` and then `[<timestamp>] created worker: <worker-id>`.
   - Testable evidence: The created-worker line includes one UUIDv6 worker ID and omits verbose accepted-worker internals.

3. Hop 3: URL-only input protection
   - User action: A user tries to create a worker with a relative path, absolute path, or local `file://` input.
   - Expected product response: The CLI or daemon rejects the request with an understandable URL-only message and no worker is created.
   - Testable evidence: Rejected inputs produce no created-worker line, no worker table entry, and no allocated worker ID.

4. Hop 4: Operator lifecycle visibility
   - User action: A daemon operator watches the daemon terminal during worker creation, user-message activity, agent status updates, repo clone start, stream start, failures, and completion.
   - Expected product response: The daemon emits concise timestamped string messages for those lifecycle moments.
   - Testable evidence: A representative feature run produces one readable timestamped line per required lifecycle category.

5. Hop 5: Worker list scanability
   - User action: An agent workflow maintainer runs the worker/agent listing command.
   - Expected product response: The CLI prints connection lifecycle lines followed by a simple table with worker status, worker id, and git repo in the existing listing order.
   - Testable evidence: The table has clear headers, includes UUIDv6 worker IDs, includes the git repo value, and keeps the known existing ordering.

6. Hop 6: Stream startup clarity
   - User action: A user starts the CLI event stream.
   - Expected product response: The stream begins with `[<timestamp>] started` and does not add separate connecting lines unless the existing stream command flow already requires them.
   - Testable evidence: Stream startup output contains the started line as the first lifecycle event line and follows the resolved default for connection-line behavior.

7. Hop 7: Fast event readability
   - User action: Daemon, agent, and worker events arrive quickly while a CLI stream is active.
   - Expected product response: Each event is rendered as a complete timestamped string line, including clone activity as `cloning repo: <repo-url>` and subsequent action/event strings.
   - Testable evidence: A fast burst of lifecycle events remains line-oriented, ordered according to the stream's observable event order, and free of nested structured-log formatting.

8. Hop 8: Low-overhead lifecycle completion
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
- Success: Daemon terminal output includes timestamped lifecycle messages for user-message activity, agent status updates, worker creation, repo clone start, stream start, relevant failures, completion, and comparable daemon/agent lifecycle moments.
- Success: Worker IDs visible in CLI output, daemon logs, event stream output, and worker tables are UUIDv6 values.
- Failure: Relative local paths are rejected for worker creation with an understandable URL-only message and no worker is created.
- Failure: Absolute local paths are rejected for worker creation with an understandable URL-only message and no worker is created.
- Failure: Local `file://` inputs are rejected for worker creation with an understandable URL-only message and no worker is created.
- Failure: Daemon-side worker creation rejects non-URL repository inputs even when the caller is not the CLI.
- Empty state: Worker/agent listing with no workers still prints connection lifecycle lines and a readable empty table or established empty-list representation with the required headers.
- Empty state: Event streaming can start and emit `[<timestamp>] started` even before later lifecycle events arrive.
- Edge state: Fast event bursts remain line-oriented, with one complete terminal line per event and no nested structured-log output.
- Edge state: If the existing stream command flow already requires connection lifecycle lines, those lines may appear before stream events; otherwise stream output starts with `[<timestamp>] started`.
- Edge state: Lifecycle logging remains low-noise by avoiding verbose protocol payloads, accepted-worker internals, or rich TUI redraw behavior.
- Regression guard: Existing worker lifecycle semantics are unchanged except for logging, UUIDv6 IDs, output rendering, and URL-only repo validation.

## Product alternatives and tournament

This scale candidate would argue against a rich structured logging alternative because JSON, nested structured logs, or protocol payloads would contradict the prompt non-goal and increase operator parsing burden. It would also argue against a rich TUI alternative because dynamic rendering is less predictable under fast event rates and outside the accepted scope.

The credible competing product path is a leaner candidate that only changes the `feature` command and listing output while leaving daemon/operator lifecycle coverage minimal. This candidate accepts slightly broader lifecycle coverage because the prompt explicitly requires daemon logs for user-message activity, agent status updates, worker creation, clone start, stream start, failures, completion, and comparable moments. The broader coverage is still contained by the low-overhead, timestamp-plus-message format.

Expected tournament differentiator: scale_v1 should score strongly on NFR feasibility, fast event readability, daemon/operator predictability, and scope containment because it avoids richer rendering and keeps every output requirement line-oriented and testable.

## Evolution summary

This is a seed PM draft rather than a final evolved champion. It preserves the prompt's product value and resolved defaults while emphasizing product constraints that make the feature reliable under fast event rates:

- Line-oriented output is a product requirement, not just an implementation preference.
- Lifecycle messages are concise and bounded to reduce operator noise.
- Stream startup, clone activity, failure, and completion behavior are explicitly covered.
- URL-only validation covers both CLI and daemon/API callers.
- UUIDv6 worker IDs are required anywhere worker IDs are created, logged, serialized, streamed, displayed, or listed.

No blocking product question remains in this draft.

## Success measures

- A valid `feature` command produces exactly the expected lifecycle sequence for connection and worker creation, with a UUIDv6 worker ID and without verbose accepted-worker detail.
- Worker/agent listing output includes the expected connection lines and a readable table with worker status, worker id, and git repo.
- CLI event stream output begins with `[<timestamp>] started` and renders each subsequent event as one timestamped line.
- Representative daemon lifecycle scenarios produce terminal-visible timestamped messages for all mandatory lifecycle categories.
- Path-like repository inputs are rejected before worker creation in both CLI and daemon/API entry points.
- Tests or validation scenarios cover success, failure, empty, fast-event, and ordering cases for the required lifecycle outputs.
- Operator-facing output remains plain text and readable without JSON decoding, rich terminal support, or verbose protocol inspection.

## Risks and compliance

- Risk: Fast event rates could produce too much terminal noise if every low-level event becomes a lifecycle line.
  - Product constraint: Only lifecycle-relevant messages should be emitted, and messages should be concise and bounded.

- Risk: Rejecting local paths may break users who previously depended on local repository inputs.
  - Product constraint: The CLI-facing failure must clearly explain that worker repositories must be cloneable URLs, and no worker should be created for rejected inputs.

- Risk: UUIDv6 adoption could create inconsistent worker ID displays if some surfaces continue showing old ID formats.
  - Product constraint: Worker IDs must be UUIDv6 across CLI lifecycle output, daemon logs, worker tables, serialized worker references, and displayed event output.

- Risk: The `clonning repo` example could be implemented literally despite the resolved default.
  - Product constraint: Event-stream repo-clone output should use corrected spelling: `cloning repo`.

- Compliance and data handling: This feature should not introduce additional sensitive data exposure in terminal logs. Lifecycle lines should avoid verbose protocol payloads and should include only progress-relevant fields such as worker ID, status, git repo URL, and concise event message text.

## Open questions

None. The prompt records that the user approved the proposed defaults on 2026-06-02.

