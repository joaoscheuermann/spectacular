# PRD Evolution PM

- Receipt id: `prd_evolution_pm_01`
- Role: evolution PM
- Phase goal: Graft the top two PRD candidates into Current Champion v1 and write the Champion PRD.

## Inputs read

- `.doric/06-2026/02/20260602-165401-daemon_cli_lifecycle_logs/PROMPT.md`
- `.agents/skills/doric/references/02-prd-generation.md`
- `.doric/06-2026/02/20260602-165401-daemon_cli_lifecycle_logs/agents/001_prd_pm_security_candidate.md`
- `.doric/06-2026/02/20260602-165401-daemon_cli_lifecycle_logs/agents/003_prd_pm_scale_candidate.md`
- `.doric/06-2026/02/20260602-165401-daemon_cli_lifecycle_logs/agents/005_prd_tournament_evaluator.md`
- `.doric/06-2026/02/20260602-165401-daemon_cli_lifecycle_logs/logs/prd_tournament_evals.json`

## Tournament result used

The tournament ranked the candidates as:

1. `scale_v1`, final Elo `1031.233`, record `2-0`.
2. `security_v1`, final Elo `999.264`, record `1-1`.
3. `lean_v1`, final Elo `969.503`, record `0-2`.

The Step 02 concept-grafting input pair is therefore:

- Base Champion source: `scale_v1`.
- Compatible graft source: `security_v1`.

## Grafting method

I treated `scale_v1` as the base candidate because it won the tournament and best covers fast-event readability, bounded output, low-overhead lifecycle lines, failure/completion visibility, stream startup behavior, URL-only validation, and UUIDv6 identity.

I grafted only product-scope concepts from `security_v1` that strengthen the base without adding implementation design or widening into a compliance platform. I preserved `lean_v1` only as a low-friction influence using the tournament evaluator's summary: existing command flows, concise messages, simple tables, understandable validation failures, and no new wizard or rich interface.

## Grafted concepts

- Credential redaction across daemon logs, CLI lifecycle output, listing tables, and event streams when repository URLs or user-derived messages contain credentials, API keys, tokens, passwords, or comparable secret material.
- Privacy and audit safeguards bounded to terminal lifecycle output, failure/completion visibility, and URL-only validation.
- Safe one-line terminal rendering for user-derived lifecycle content containing line breaks or terminal control characters.
- Terminal-output integrity requirements so one source event cannot visually forge additional lifecycle lines.
- Local-data exposure controls for rejected path-like inputs and local `file://` inputs.
- Clear failure messaging that preserves the low-friction UX goal while enforcing URL-only repository input.

## Concepts rejected

- Full structured tracing, JSON logs, nested log formats, rich TUI rendering, progress spinners, and terminal redraw behavior were rejected because they contradict the prompt's plain timestamp-plus-message constraint.
- Durable audit-log storage, centralized compliance reporting, access-control changes, and encryption features were rejected as beyond product scope.
- Local path or local `file://` repository support was rejected because the resolved prompt decision requires URL-only worker repository input.
- Raw credential-bearing URLs, raw local paths from rejected inputs, and unescaped multiline/control-character content were rejected because they undermine privacy and terminal-output integrity.
- A lean-only narrow scope that changes only the `feature` command and listing output was rejected because the prompt explicitly requires daemon lifecycle coverage for user-message activity, agent status updates, worker creation, clone start, stream start, failures, completion, and comparable daemon/agent moments.
- New guided workflows, wizards, or additional command surfaces were rejected because the compatible lean influence is low-friction behavior in existing flows, not new UX surface area.

## Champion PRD produced

Champion version: Current Champion v1.

Output file:

- `.doric/06-2026/02/20260602-165401-daemon_cli_lifecycle_logs/PRD.md`

The Champion PRD uses exactly the Step 02 schema:

- `# Product Requirements Document`
- `## Problem statement`
- `## Goals`
- `## Non-goals`
- `## Personas`
- `## Generator persona debate`
- `## Primary workflows`
- `## Functional requirements`
- `## User Value Hops`
- `## Acceptance criteria`
- `## Product alternatives and tournament`
- `## Evolution summary`
- `## Success measures`
- `## Risks and compliance`
- `## Open questions`

## PRD sections changed

- Problem statement
- Goals
- Non-goals
- Generator persona debate
- Primary workflows
- Functional requirements
- User Value Hops
- Acceptance criteria
- Product alternatives and tournament
- Evolution summary
- Success measures
- Risks and compliance
- Open questions

## Product-scope guardrails

- The PRD avoids technical implementation design beyond product-scope feasibility constraints.
- Architecture requirements from `PROMPT.md` are used only where they affect user-visible behavior, feasibility, validation, or compliance boundaries.
- The champion does not invent unsupported `lean_v1` features; it only preserves low-friction wording and command-flow constraints that are already supported by the tournament summary.

## Blockers

None. The tournament evaluator identified `scale_v1` and `security_v1` as the top two candidates, the grafted concepts are compatible with the prompt and non-goals, and the Champion PRD has been written.
