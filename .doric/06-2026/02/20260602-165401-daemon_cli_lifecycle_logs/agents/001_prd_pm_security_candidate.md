# Product Requirements Document

- Candidate id: security_v1
- Generator persona: PM security/compliance advocate

## Problem statement

Doric daemon and CLI lifecycle activity is currently harder to audit than it should be because key moments are either too noisy, not visible in a consistent terminal format, or tied to verbose worker details that are not useful for day-to-day operators. Users need concise lifecycle lines that show connection, worker creation, repository activity, agent status, event-stream progress, failures, and completion without leaking credentials, accepting unsafe local repository inputs, or changing existing worker lifecycle semantics.

## Goals

- Show concise timestamped lifecycle output for daemon, CLI, worker, agent, and event-stream moments that matter to users and operators.
- Replace verbose `feature` worker-acceptance output with the approved lifecycle sequence while still surfacing the created worker ID.
- Make worker and agent listing output readable by showing connection lifecycle lines followed by a simple worker table.
- Make event streams auditable through timestamped line-oriented messages, beginning with `started`.
- Ensure worker IDs displayed, logged, serialized, and tabled for this flow are UUIDv6 values.
- Reject local repository paths and `file://` inputs for worker creation, accepting only cloneable repository URL syntax.
- Minimize privacy and credential exposure in lifecycle output by redacting secrets from repository URLs and user-derived messages before terminal display.

## Non-goals

- Do not redesign the full daemon logging or tracing system beyond lifecycle terminal logs needed for this feature.
- Do not introduce JSON, rich TUI rendering, or nested structured logs for the requested lifecycle output.
- Do not change worker lifecycle semantics except for logging, UUIDv6 IDs, output rendering, and URL-only repo validation.
- Do not allow local filesystem paths, including relative paths, absolute paths, or file URLs that behave as local paths, as worker repo inputs.
- Do not require durable audit-log storage, centralized compliance reporting, or access-control changes as part of this product scope.
- Do not authorize technical design, decomposition, tests, or implementation changes from this seed candidate alone.

## Personas

- Feature-run initiator: runs Doric CLI commands such as `feature` and wants concise progress output.
- Daemon operator: watches the daemon terminal and needs lifecycle moments to be visible.
- Agent workflow maintainer: verifies worker creation, event streaming, and status changes without parsing noisy accepted-worker output.
- Implementation engineer: updates daemon/CLI boundaries, worker ID generation, repo validation, and tests while preserving existing workflow semantics.

## Generator persona debate

Security/compliance position: lifecycle logs should be useful audit breadcrumbs while disclosing the least practical information. The candidate therefore emphasizes URL-only worker inputs, credential redaction, one-line rendering of user-derived event text, failure visibility, and UUIDv6 traceability.

Persona-specific tradeoffs accepted:

- Accepted slight UX friction by rejecting local paths that may have worked informally before, because URL-only input is an explicit resolved decision and prevents ambiguous local repo handling.
- Accepted a narrower audit surface than a full structured tracing system, because the prompt requires straightforward timestamp-plus-message terminal logs and excludes JSON or rich TUI output.
- Accepted redacted repository display over full raw URL display, because operator auditability does not require exposing embedded credentials.
- Accepted preserving existing listing order and lifecycle semantics even when security-focused sorting or state normalization could make audits easier, because those would widen scope.

Self-evaluation score: 8/10. This draft strongly covers safety, privacy, and auditability, but leaves exact redaction tokens and timestamp rendering details to technical design so it does not overstep product scope.

## Primary workflows

1. A feature-run initiator starts `feature` with an accepted repository URL and sees concise connection and worker-creation lifecycle lines instead of verbose accepted-worker detail.
2. A daemon operator watches the daemon terminal and sees timestamped lifecycle messages for user messages, agent status updates, worker creation, clone start, stream start, failures, and completion.
3. An agent workflow maintainer lists workers or agents and sees daemon connection lifecycle lines followed by a simple table with worker status, worker id, and git repo.
4. A CLI user streams events and sees timestamped event messages, beginning with `started` and including `cloning repo: <repo-url>` when repository cloning begins.
5. A user attempts worker creation with a relative path, absolute path, or local `file://` input and receives an understandable rejection before any worker is created.
6. A user or automation supplies a repository URL or message containing credentials, and terminal output redacts secret material while preserving enough non-secret context for troubleshooting.

## Functional requirements

1. Daemon and CLI lifecycle lines must use a straightforward timestamp-plus-message format. If the repository already has a timestamp style, use it; otherwise use a compact ISO-8601/RFC3339-style timestamp.
2. The `feature` command must replace verbose accepted-worker detail with these lifecycle lines:
   - `[<timestamp>] connecting to daemon...`
   - `[<timestamp>] connected to daemon...`
   - `[<timestamp>] creating worker...`
   - `[<timestamp>] created worker: <worker-id>`
3. The `created worker` value must be a UUIDv6 worker ID.
4. Worker IDs shown in CLI output, daemon logs, event messages, persisted or serialized worker references, and worker tables must be UUIDv6 values for newly created workers in this scope.
5. The worker or agent listing command must show:
   - `[<timestamp>] connecting to daemon...`
   - `[<timestamp>] connected to daemon...`
   - A simple table with clear headers for worker status, worker id, and git repo.
6. Worker listing output must preserve the existing listing order.
7. CLI event streaming must render timestamped lifecycle/event lines, beginning with `[<timestamp>] started`.
8. Repo clone event output must use the corrected spelling `cloning repo: <repo-url>`.
9. Streamed lifecycle messages must include subsequent action or event strings as timestamped lines without switching to JSON, nested logs, or a rich TUI.
10. Daemon terminal logs must cover user-message activity, agent status updates, worker creation, clone start, stream start, failures when relevant, completion, and comparable daemon/agent lifecycle moments.
11. Worker creation must accept cloneable repository URL syntax only.
12. Worker creation must reject relative paths, absolute paths, local path-like inputs, and local `file://` inputs.
13. URL-only validation failures must be understandable to CLI users and must not create a worker.
14. URL-only validation must protect both CLI users and non-CLI callers of worker creation.
15. Repository URLs and user-derived lifecycle messages displayed in daemon logs, CLI lifecycle output, listing tables, or event streams must not expose embedded credentials, API keys, tokens, passwords, or local filesystem paths.
16. User-derived lifecycle message content must not be able to forge extra terminal lifecycle lines; line breaks and control characters should render safely as a single auditable event line or be escaped.
17. Existing worker lifecycle behavior must be preserved except where logging, ID generation, output formatting, or repo validation must change.

## User Value Hops

1. Hop 1: Connection visibility.
   - Trigger: A user runs `feature` or the worker/agent listing command.
   - Expected value: The CLI immediately shows `[<timestamp>] connecting to daemon...` followed by `[<timestamp>] connected to daemon...`.
   - Test signal: The first lifecycle lines are present in order and do not include verbose accepted-worker details.
2. Hop 2: Safe worker creation visibility.
   - Trigger: A user runs `feature` with an accepted repository URL.
   - Expected value: The CLI shows `[<timestamp>] creating worker...` followed by `[<timestamp>] created worker: <worker-id>`.
   - Test signal: The worker ID is UUIDv6, the output is concise, and no raw accepted-worker payload is printed.
3. Hop 3: URL-only guardrail.
   - Trigger: A user runs worker creation with a relative path, absolute path, local path-like value, or `file://` URL.
   - Expected value: The CLI explains that repository input must be a URL and does not create a worker.
   - Test signal: No worker ID is emitted, no created-worker lifecycle line appears, and daemon-side worker creation is not reached for invalid inputs.
4. Hop 4: Daemon lifecycle audit line.
   - Trigger: The daemon handles user messages, agent status changes, worker creation, clone start, stream start, failures, or completion.
   - Expected value: The daemon terminal shows one timestamped string message per lifecycle moment.
   - Test signal: Required lifecycle moments produce line-oriented timestamp-plus-message logs without JSON, nested formatting, or credential leakage.
5. Hop 5: Streamed event observability.
   - Trigger: A CLI user starts event streaming.
   - Expected value: The stream begins with `[<timestamp>] started`, then renders clone, action, failure, completion, or other event strings as timestamped lines.
   - Test signal: `cloning repo: <repo-url>` appears when clone starts, subsequent events remain ordered and line-oriented, and connection lines are absent unless the existing stream command flow already requires them.
6. Hop 6: Worker table scanability.
   - Trigger: A user lists workers or agents.
   - Expected value: After connection lifecycle lines, the user sees a simple table with worker status, worker id, and git repo.
   - Test signal: Table headers are clear, existing listing order is preserved, UUIDv6 IDs are shown, and empty listings remain understandable.
7. Hop 7: Credential and terminal-output safety.
   - Trigger: A repository URL or event message contains credentials, tokens, passwords, control characters, or line breaks.
   - Expected value: Terminal output remains useful for audit and support without exposing secrets or forged extra lifecycle lines.
   - Test signal: Secret-bearing inputs are redacted in all lifecycle surfaces, and each source event renders as one safe displayed line.

## Acceptance criteria

- Success: Running `feature` with an accepted repository URL prints the approved four lifecycle lines in order and includes a UUIDv6 worker ID in `created worker: <worker-id>`.
- Success: `feature` output no longer prints verbose accepted-worker detail.
- Success: Daemon terminal output uses timestamp-plus-message lines for user-message activity, agent status updates, worker creation, clone start, stream start, failures, completion, and comparable lifecycle moments.
- Success: Worker or agent listing prints connection lifecycle lines followed by a simple table with status, worker id, and git repo headers.
- Success: Worker listing preserves the existing listing order.
- Success: Empty worker listings still show connection lifecycle lines and an understandable empty table or the repository's existing empty-table convention.
- Success: Event streaming starts with `[<timestamp>] started` and renders later event strings as timestamped lines.
- Success: Repository clone stream output uses `cloning repo: <repo-url>`.
- Success: Newly created worker IDs in lifecycle output, daemon logs, event streams, serialized worker references, and worker tables are UUIDv6 values.
- Failure: Relative paths, absolute paths, local path-like values, and local `file://` inputs are rejected before worker creation.
- Failure: URL-only validation failures explain that repository input must be a URL and do not emit a created-worker line.
- Failure: Non-CLI callers cannot bypass URL-only validation to create a worker from a local path.
- Privacy: Repository URLs and user-derived messages containing credentials, tokens, API keys, or passwords do not display those secrets in daemon logs, CLI lifecycle output, listing tables, or event streams.
- Privacy: Local filesystem paths from rejected inputs are not echoed in a way that discloses unnecessary local machine details.
- Edge: Fast event bursts remain readable as ordered, line-oriented terminal output.
- Edge: User-derived event text containing line breaks or terminal control characters cannot visually forge additional lifecycle lines.
- Constraint: The feature does not introduce JSON logs, rich TUI rendering, nested structured logs, lifecycle semantic changes, or local-path repo support.

## Product alternatives and tournament

No pairwise tournament has been run by this seed child agent. This candidate is intentionally differentiated by a security/compliance stance:

- Lean-UX alternative likely minimizes validation and redaction language to keep CLI output as simple as possible.
- Scale/performance alternative likely emphasizes event throughput, buffering, and latency budgets for streaming.
- Security/compliance alternative, represented by `security_v1`, prioritizes safe input handling, secret redaction, terminal-output integrity, and audit coverage while preserving the prompt's simple terminal-output model.

The main PRD evolution process should compare whether the added redaction and terminal-output safety requirements improve the final champion without creating unacceptable implementation scope.

## Evolution summary

This is the first security/compliance seed draft. It preserves the approved product defaults from `PROMPT.md`, including UUIDv6 worker IDs, URL-only repo input, rejection of local and `file://` inputs, the approved `feature` lifecycle sequence, listing table columns, stream start behavior, corrected `cloning repo` spelling, mandatory lifecycle coverage, existing timestamp-style fallback, and existing listing order. It adds product-scope security acceptance criteria for credential redaction and safe one-line rendering because those are necessary for privacy, auditability, and safe terminal logs.

## Success measures

- 100% of accepted `feature` worker-creation runs show the approved lifecycle sequence and no verbose accepted-worker detail.
- 100% of newly created worker IDs displayed or logged in this scope are UUIDv6.
- 100% of local relative paths, absolute paths, local path-like inputs, and local `file://` inputs tested for worker creation are rejected without creating a worker.
- 100% of required lifecycle categories have timestamped daemon terminal log coverage in acceptance testing.
- 100% of event streams begin with a timestamped `started` line and render subsequent events as timestamped string messages.
- 100% of listing-command acceptance checks show connection lifecycle lines followed by worker status, worker id, and git repo columns.
- 0 known credential-bearing fixture values are exposed in daemon logs, CLI lifecycle output, listing tables, or event streams.

## Risks and compliance

- Credential leakage risk: Repository URLs may contain credentials or tokens. Product acceptance must require redaction before terminal display.
- Local data exposure risk: Allowing local paths would expose unsupported local filesystem handling and ambiguous repo provenance. Product scope requires URL-only inputs and rejection of local `file://` values.
- Audit integrity risk: User-derived messages with line breaks or control characters could visually forge lifecycle lines. Product scope requires safe one-line rendering or escaping.
- Incomplete lifecycle risk: Missing failure or completion lines would reduce operator trust. Required lifecycle coverage includes failures and completion when relevant.
- Over-logging risk: A security-heavy audit trail could become noisy or leak details. This candidate preserves concise timestamp-plus-message output and rejects JSON, rich TUI, nested structured logs, and full tracing redesign.
- Backward-compatibility risk: Users who relied on local paths will be blocked. This is an accepted product tradeoff because URL-only repository inputs are an explicit resolved decision.

## Open questions

None blocking. The prompt records that all open questions were resolved on 2026-06-02 and the user approved the proposed defaults.
