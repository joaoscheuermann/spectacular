# Product Requirements Document

## Problem statement

Candidate id: `lean_v1`

Doric daemon and CLI lifecycle activity is currently harder to follow than it should be for ordinary feature-run operators. Important moments such as daemon connection, worker creation, repository cloning, stream start, user-message activity, agent status updates, failures, and completion are either hidden, too verbose, or presented through worker/protocol detail that increases cognitive load.

The product need is a cleaner lifecycle experience: terminal output should show concise timestamped lines that answer "what is Doric doing now?" without requiring users to parse accepted-worker internals, structured logs, or ambiguous repository inputs.

## Goals

- Make key daemon, agent, worker, and event-stream lifecycle activity readable in ordinary terminal output.
- Replace verbose `feature` accepted-worker detail with short lifecycle lines that still show the created worker ID.
- Make worker listing output easier to scan by showing daemon connection lifecycle lines followed by a simple worker table.
- Render streamed daemon and agent events as timestamped lifecycle/event messages.
- Make worker IDs visible as stable, time-sortable UUIDv6 values across CLI output, daemon logs, and worker tables.
- Reduce setup ambiguity by accepting repository URLs only for worker creation and rejecting local paths before a worker is created.
- Preserve existing command and lifecycle semantics except where user-visible logging, output formatting, UUIDv6 worker IDs, and URL-only repository validation are required.

## Non-goals

- Do not redesign the full daemon logging or tracing system beyond lifecycle terminal logs needed for this feature.
- Do not introduce JSON, rich TUI rendering, or nested structured logs for requested lifecycle output.
- Do not change worker lifecycle semantics except for logging, UUIDv6 IDs, output rendering, and URL-only repo validation.
- Do not allow local filesystem paths, including relative paths, absolute paths, or local `file://` inputs, as worker repo inputs.
- Do not add new CLI commands as a requirement for this feature.
- Do not require users to provide extra inputs only to obtain cleaner lifecycle output.

## Personas

- Feature-run initiator: runs Doric CLI commands such as `feature` and wants concise progress output.
- Daemon operator: watches the daemon terminal and needs lifecycle moments to be visible.
- Agent workflow maintainer: verifies worker creation, event streaming, and status changes without parsing noisy accepted-worker output.
- Implementation engineer: needs product requirements that preserve command semantics while clarifying output, validation, and test expectations.

## Generator persona debate

`lean_v1` is drafted from the PM lean-UX advocate perspective. It optimizes for low-friction CLI workflows, minimal cognitive load, clean terminal readability, and obvious recovery from invalid repository inputs.

Self-evaluation score: 9/10.

Persona-specific tradeoffs accepted:

- Prefer plain timestamped lines and simple tables over richer terminal UI, because operator readability and script-friendly output matter more than visual density for this request.
- Keep lifecycle text concise even if that means less diagnostic detail in the primary CLI path; deeper diagnostics can remain outside this product scope.
- Treat URL-only repository validation as a clear user-facing boundary even though it removes previously possible local-path workflows.
- Preserve existing listing order and command flows to avoid adding learning cost while improving output clarity.
- Correct the user-provided event example to `cloning repo` because the prompt resolved this spelling decision.

## Primary workflows

1. Feature-run initiator starts a feature run.
   The CLI shows `[<timestamp>] connecting to daemon...`, `[<timestamp>] connected to daemon...`, `[<timestamp>] creating worker...`, and `[<timestamp>] created worker: <worker-id>`.

2. Daemon operator watches lifecycle activity.
   The daemon terminal emits timestamp-plus-message lines for user-message activity, agent status updates, worker creation, clone start, stream start, failures when relevant, completion, and comparable daemon/agent lifecycle moments.

3. User lists workers or agents.
   The CLI shows daemon connection lifecycle lines, then a simple table with clear headers for worker status, worker id, and git repo, preserving the existing listing order.

4. User streams events.
   The stream begins with `[<timestamp>] started` and then renders each daemon/agent event as a timestamped string message, including repository clone activity such as `[<timestamp>] cloning repo: https://foo.bar`.

5. User provides an invalid repository input.
   If the input is a relative path, absolute path, or local `file://` input, the CLI or daemon-facing workflow rejects it with an understandable error and no worker is created.

## Functional requirements

1. Lifecycle line format
   - Daemon terminal logs and CLI lifecycle lines must use a straightforward `[<timestamp>] <message>` shape.
   - Timestamp format should use the repository's existing timestamp style if one exists; otherwise it should use a compact ISO-8601/RFC3339-style timestamp.
   - Lifecycle messages must remain line-oriented and readable when events arrive quickly.

2. Daemon lifecycle visibility
   - Daemon terminal logs must cover user-message activity, agent status updates, worker creation, clone start, stream start, relevant failures, completion, and comparable lifecycle moments.
   - Daemon log lines must be readable without JSON, rich TUI rendering, or nested structured-log decoding.

3. `feature` command lifecycle output
   - The `feature` command must show, in order:
     - `[<timestamp>] connecting to daemon...`
     - `[<timestamp>] connected to daemon...`
     - `[<timestamp>] creating worker...`
     - `[<timestamp>] created worker: <worker-id>`
   - The command must not show the current verbose accepted-worker detail in the normal successful path.
   - The created worker ID must be visible in the final lifecycle line.

4. Worker listing output
   - The listing workflow must show connection lifecycle lines before listing content:
     - `[<timestamp>] connecting to daemon...`
     - `[<timestamp>] connected to daemon...`
   - The listing workflow must render a simple table with clear headers for worker status, worker id, and git repo.
   - The table must preserve the existing listing order.
   - Empty listing output must still show a clear table or empty-state row without reverting to verbose worker detail.

5. Event-stream output
   - Stream output must begin with `[<timestamp>] started`.
   - Streamed daemon/agent events must render as timestamped string messages.
   - Repo clone activity must render as a readable event such as `[<timestamp>] cloning repo: https://foo.bar`.
   - Stream output must not add separate connecting lines unless the existing stream command flow already requires connection lifecycle lines.

6. Worker ID visibility
   - Worker IDs shown in CLI output, daemon logs, and worker tables must be UUIDv6 values.
   - Worker IDs must be stable for the worker they identify and suitable for time-sortable lifecycle reading.

7. Repository input validation
   - Worker creation must accept cloneable repository URL syntax.
   - Worker creation must reject local relative paths, local absolute paths, and local `file://` inputs.
   - URL-only validation must be enforced for CLI users and for non-CLI worker-creation callers.
   - Any rejected path-like input must produce an understandable CLI-facing error and must not create a worker.

8. Scope preservation
   - Existing worker lifecycle behavior, command names, and daemon workflows should remain in place except where this PRD explicitly changes visible logging, output rendering, UUIDv6 worker IDs, or URL-only repository validation.

## User Value Hops

1. Hop 1: User sees daemon connection progress.
   - Trigger: a feature-run or listing command starts.
   - User-visible result: the CLI prints timestamped connecting and connected lines.
   - Test: command output contains those lines in order and does not require parsing verbose worker detail.

2. Hop 2: User sees worker creation progress.
   - Trigger: the `feature` command requests a worker.
   - User-visible result: the CLI prints timestamped creating-worker and created-worker lines.
   - Test: created-worker line includes a UUIDv6 worker ID and the old verbose accepted-worker detail is absent.

3. Hop 3: Operator can follow daemon lifecycle moments.
   - Trigger: user-message activity, agent status updates, worker creation, clone start, stream start, failure, or completion occurs.
   - User-visible result: the daemon terminal prints a timestamp-plus-message lifecycle line.
   - Test: each mandatory lifecycle moment has a readable line in daemon terminal output.

4. Hop 4: User scans worker state.
   - Trigger: a worker/agent listing command completes daemon connection.
   - User-visible result: output shows a simple table with worker status, worker id, and git repo.
   - Test: populated and empty worker lists render cleanly with clear headers and preserve existing listing order.

5. Hop 5: User follows streamed events.
   - Trigger: event streaming begins and receives daemon/agent events.
   - User-visible result: stream starts with `[<timestamp>] started` and subsequent events are timestamped string messages.
   - Test: stream output includes start, clone, and arbitrary event messages without nested structured output.

6. Hop 6: User recovers from invalid repository input.
   - Trigger: user provides a relative path, absolute path, or local `file://` input for worker creation.
   - User-visible result: the request is rejected with a clear URL-only error and no worker is created.
   - Test: path-like inputs fail before worker creation and explain that repository URLs are required.

7. Hop 7: Maintainer verifies consistent worker identity.
   - Trigger: worker ID appears in CLI output, daemon logs, and tables.
   - User-visible result: every displayed worker ID uses UUIDv6 format for the same worker.
   - Test: all visible and serialized worker ID surfaces for a created worker match the UUIDv6 expectation.

## Acceptance criteria

- Success: `feature` prints the four required lifecycle lines in order, includes `created worker: <worker-id>`, uses a UUIDv6 worker ID, and suppresses verbose accepted-worker detail.
- Success: daemon terminal logs show timestamp-plus-message lines for user-message activity, agent status updates, worker creation, clone start, stream start, relevant failures, and completion.
- Success: worker listing prints connection lifecycle lines followed by a table with worker status, worker id, and git repo headers.
- Success: event streaming begins with `[<timestamp>] started` and renders timestamped string messages for clone activity and subsequent events.
- Success: repository URL inputs accepted by worker creation continue to create workers and display the lifecycle output.
- Failure: relative paths, absolute paths, and local `file://` inputs are rejected with a clear URL-only error and do not create workers.
- Failure: daemon or stream failures surface as readable timestamped lifecycle messages when relevant.
- Empty state: worker listing with no workers still renders cleanly with clear headers or an explicit empty row/message, without verbose accepted-worker detail.
- Edge state: rapid lifecycle events remain line-oriented and readable.
- Edge state: existing stream command connection behavior is preserved; additional connecting lines are not required unless already part of that flow.
- Edge state: listing order remains unchanged from the existing behavior.

## Product alternatives and tournament

This is a seed candidate, so no cross-candidate tournament has been run by this child agent. The lean-UX product stance is:

- Preferred path: plain timestamped lifecycle lines plus a simple listing table. This best satisfies low cognitive load, operator readability, and minimal workflow friction.
- Rejected alternative: JSON or structured event output as the primary display. It would improve machine parsing but violates the prompt's clean terminal readability constraint.
- Rejected alternative: rich TUI output. It may increase visual affordance but adds complexity and conflicts with the non-goal against rich TUI rendering.
- Rejected alternative: preserving local-path worker creation as a compatibility mode. It reduces migration friction but contradicts the resolved URL-only repository decision.

## Evolution summary

`lean_v1` turns the prompt into a low-friction product path centered on ordinary terminal readability:

- It narrows the main CLI experience to predictable lifecycle lines.
- It preserves existing command flows and listing order.
- It treats invalid repository inputs as a recoverable CLI error, not an operator mystery.
- It keeps UUIDv6 worker identity as a user-visible consistency requirement without prescribing implementation mechanics.
- It records output examples and acceptance criteria so downstream technical design can select repository-specific formatting and validation mechanisms.

## Success measures

- `feature` normal successful output contains the required lifecycle lines and zero verbose accepted-worker detail.
- 100% of mandatory daemon lifecycle moments have timestamp-plus-message terminal coverage in tests or validation evidence.
- 100% of displayed worker IDs in changed workflows are UUIDv6 values.
- Worker listing output includes status, worker id, and git repo columns in all covered populated and empty states.
- Event stream validation covers start, clone, and arbitrary event message rendering.
- Path-like worker repo inputs are rejected in covered CLI and non-CLI worker-creation paths, with no worker created.
- Manual operator review can identify daemon connection, worker creation, repo clone, stream start, failure, and completion moments from terminal output without inspecting protocol detail.

## Risks and compliance

- Compatibility risk: users who relied on local path worker creation will now receive an error. The PRD accepts this tradeoff because URL-only repository input is a resolved requirement.
- Readability risk: too many lifecycle events could become noisy. The product boundary is mandatory lifecycle coverage with concise line-oriented messages, not full tracing.
- Consistency risk: timestamp and message formatting may diverge between daemon logs and CLI output. Product acceptance requires a consistent visible shape while allowing the repository's existing timestamp style.
- Identity risk: mixed old and new worker ID formats would undermine operator readability. Product acceptance requires visible worker IDs for this feature to be UUIDv6.
- Recovery risk: unclear URL-only errors would increase support cost. Rejection messages must be understandable to CLI users and must state that repository URLs are required.
- Compliance/privacy note: rejecting local path and local `file://` inputs helps avoid exposing local filesystem details in worker lifecycle flows.

## Open questions

None. The prompt records that all open questions were raised, the user approved the proposed defaults on 2026-06-02, and the defaults are resolved decisions for this PRD candidate.
