# Prompt

## Feature summary

Doric should make daemon and CLI lifecycle activity easier to follow by emitting concise terminal log lines for key daemon, agent, worker, and event-stream moments. The CLI should replace verbose worker-acceptance output with short lifecycle lines for `feature`, show clean connection logs plus a worker table when listing workers, and stream timestamped event messages. Worker creation should use UUIDv6 identifiers, and workers should accept repository URLs only, not filesystem paths.

## User value

- CLI users can see what Doric is doing without reading verbose protocol or worker details.
- Operators can follow daemon and agent lifecycle activity directly in the terminal.
- Worker identifiers are stable, time-sortable UUIDv6 values that are suitable for lifecycle logs and tables.
- Repository inputs are less ambiguous because worker creation accepts URLs and rejects local paths.

## Business or commercial driver

Cleaner lifecycle logging reduces operator confusion and support/debugging cost for Doric feature runs. URL-only worker repository input also narrows unsupported setup paths, making daemon behavior more predictable for repeatable automation.

## Personas

- Feature-run initiator: runs Doric CLI commands such as `feature` and wants concise progress output.
- Daemon operator: watches the daemon terminal and needs lifecycle moments to be visible.
- Agent workflow maintainer: verifies worker creation, event streaming, and status changes without parsing noisy accepted-worker output.
- Implementation engineer: updates daemon/CLI boundaries, worker ID generation, repo validation, and tests while preserving existing workflow semantics.

## Constraints

- Keep prompt-phase work scoped to this source-of-truth artifact; do not advance to PRD generation here.
- Daemon terminal log format must remain straightforward: a timestamp plus a string message.
- CLI lifecycle output must be cleaner than the current verbose accepted-worker detail.
- Worker IDs must be UUIDv6.
- Worker repository inputs must be URLs only; local absolute or relative paths are not allowed.
- Product and architecture scopes must stay separated for downstream PRD and technical-design phases.

## Product requirements

- Daemon terminal logs should be visible for user-message activity, agent status updates, and comparable daemon/agent lifecycle moments.
- Daemon log lines should use a straightforward timestamp-and-message format so they are readable in a terminal without structured-log decoding.
- The `feature` CLI command should display this lifecycle output instead of verbose accepted-worker detail:
  - `[<timestamp>] connecting to daemon...`
  - `[<timestamp>] connected to daemon...`
  - `[<timestamp>] creating worker...`
  - `[<timestamp>] created worker: <worker-id>`
- The CLI worker/agent listing command should display:
  - `[<timestamp>] connecting to daemon...`
  - `[<timestamp>] connected to daemon...`
  - A simple table with worker status, worker id, and git repo.
- CLI event streaming should display timestamped lifecycle/event lines such as:
  - `[<timestamp>] started`
  - `[<timestamp>] clonning repo: https://foo.bar`
  - `[<timestamp>] some action or event`
- Worker IDs shown in CLI output, daemon logs, and worker tables should be UUIDv6 values. This is duplicated in architecture requirements because it affects both visible product behavior and the underlying ID-generation contract.
- Worker repo inputs should accept URLs only and reject local paths. This is duplicated in architecture requirements because it affects both user-facing command behavior and daemon/API validation.
- Any validation failure for a path-like repo input should be understandable to a CLI user and should not create a worker.

## Architecture requirements

- Add or extend daemon-side terminal logging for user-message activity, agent status changes, worker creation, repo cloning, stream start, failures when relevant, and similar lifecycle events.
- Implement the timestamp-and-message formatter consistently across daemon logs and CLI lifecycle lines where shared formatting is practical. This is duplicated in product requirements because the format is part of the user-visible CLI and daemon terminal experience.
- Replace the `feature` command's verbose accepted-worker detail with the requested lifecycle renderer while still surfacing the created worker ID.
- Update worker listing output to render a simple table with worker status, worker id, and git repo after daemon connection lifecycle lines.
- Render streamed daemon/agent events as timestamped string messages in the CLI.
- Generate worker IDs as UUIDv6 at the worker creation boundary, and ensure any persisted, serialized, logged, or displayed worker ID path uses that value.
- Enforce URL-only repository inputs for worker creation. Validation should prevent local paths from reaching worker creation, and daemon-side enforcement should protect against non-CLI callers.
- Preserve existing worker lifecycle behavior except where logging, ID generation, output formatting, or repo validation must change.
- Add focused tests for CLI output formatting, worker listing table shape, event-stream rendering, UUIDv6 worker IDs, and URL-only repo validation using existing repository test patterns.

## Active listening notes

- Reflection: The requested feature is primarily a lifecycle observability and UX cleanup pass for Doric daemon and CLI workflows. The core value is concise, timestamped progress visibility without exposing noisy accepted-worker internals.
- Edge-case probe: If a user supplies a local path that previously worked as a repo argument, the new behavior should reject it before worker creation and explain that only repository URLs are accepted.
- Edge-case probe: If daemon lifecycle events arrive faster than the terminal can render them, output should remain line-oriented and readable rather than switching to structured or nested logging.
- Assumption not yet accepted: Existing command names and daemon APIs remain in place; this work changes lifecycle output, validation, and worker ID generation rather than introducing new commands.
- Assumption not yet accepted: The misspelled example text `clonning repo` is treated as user-provided output text until the coordinator confirms whether implementation should preserve it exactly or correct it to `cloning repo`.

## Resolved decisions

- Daemon terminal logs are required for user-message activity, agent status updates, and similar daemon/agent lifecycle moments.
- Log lines should be straightforward timestamp plus string messages.
- The `feature` command should show connecting, connected, creating worker, and created worker lifecycle lines.
- Created-worker output should include the worker ID in the form `created worker: <worker-id>`.
- Worker/agent listing should show connecting and connected lifecycle lines, then a simple worker table.
- The listing table must include worker status, worker id, and git repo.
- Event streaming should show timestamped event lines including started, repo-cloning activity, and subsequent action/event strings.
- Worker IDs should be UUIDv6.
- Workers should accept repository URLs only; paths are not allowed.
- Timestamp format should use the repository's existing timestamp style if one exists; otherwise use a compact ISO-8601/RFC3339-style timestamp.
- Event-stream repo-clone output should correct the example spelling to `cloning repo`.
- Mandatory lifecycle coverage includes user messages, agent status updates, worker creation, clone start, stream start, failures, and completion.
- Worker repo inputs may use cloneable URL syntax, but local paths and local `file://` inputs are rejected.
- URL-only validation should be enforced in both CLI argument handling and daemon worker-creation APIs.
- Stream output should begin with `[<timestamp>] started` without separate connecting lines unless the existing stream command flow already requires connection lifecycle lines.
- Worker listing tables should include clear headers and preserve the existing listing order.

## Open questions

None. The user explicitly approved the proposed defaults on 2026-06-02.

## Prompt-to-PRD alignment

Approved on 2026-06-02. The coordinator presented all open questions and defaults; the user responded, "I like the defaults." The accepted defaults are now recorded under Resolved decisions, and there are no blocking open questions for PRD generation.

## Non-goals

- Do not redesign the full daemon logging or tracing system beyond lifecycle terminal logs needed for this feature.
- Do not introduce JSON, rich TUI rendering, or nested structured logs for the requested lifecycle output.
- Do not change worker lifecycle semantics except for logging, UUIDv6 IDs, output rendering, and URL-only repo validation.
- Do not allow local filesystem paths, including relative paths, absolute paths, or file URLs that behave as local paths, as worker repo inputs.
- Do not generate `PRD.md`, `TDD.md`, decomposition artifacts, tests, or implementation changes during this prompt phase.
