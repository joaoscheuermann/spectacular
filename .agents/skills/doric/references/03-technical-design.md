# Step 3: Technical Design

Use this reference for the Doric architecture and technical design phase. Stop reading once the architect loop, staged evaluator loop, tournament rule, `GAPS_REPORT.md` handoff, design rubric, schema, and approval gate are clear.

## Goal

Create `<run>/TDD.md` from the architecture-scoped parts of `<run>/PROMPT.md`, `<run>/PRD.md`, current repo architecture, and relevant coding standards. Treat `TDD.md` as a Technical Design Document, not the development method. The design must expose technical assumptions as Dependency Hops and use fresh-context gap repair when evaluators find architecture problems.

Use `PROMPT.md` architecture requirements, architecture-tagged or shared open questions, and hard prompt constraints as technical-design inputs. Use `PRD.md` as the accepted product source of truth; do not re-derive product requirements from the prompt unless a hard prompt constraint was not represented in the PRD.

## Default loop

Run up to 3 technical-design epochs unless the user gives a different limit. Each epoch uses a fresh architect and fresh evaluator sub-agents for every evaluator stage. Ask the user for missing architectural constraints only when they affect irreversible decisions such as data stores, API style, deployment model, compatibility, or required libraries.

Register every architect, evaluator-stage, tournament, and gap-repair pass in `STATE.md` `Required agents` before spawning it. Record every pass as a spawn-backed `Agent receipts` row with a compact `Overview`. The coordinator updates `STATE.md` only after reviewing the result and marking accepted required-agent rows.

## Architect sub-agent

Spawn a fresh architect sub-agent with:

- Role: technical design author
- Ownership: write or draft `<run>/TDD.md`
- Inputs: architecture-scoped sections of `<run>/PROMPT.md`, `<run>/PRD.md`, current repo architecture, relevant standards, current `<run>/GAPS_REPORT.md` when repairing gaps
- Output: complete technical design draft
- Stop condition: design satisfies the schema or names blocking architecture facts

The architect must simulate a concise technical persona debate before finalizing the design:

- **Database Administrator:** argues for schema integrity, migrations, query shape, data retention, and consistency.
- **Frontend Lead:** argues for rendering cost, bundle/runtime size, UI state, user feedback, and client compatibility when relevant.
- **SecOps Specialist:** argues for vulnerability vectors, trust boundaries, authorization, secrets, auditability, and operational blast radius.

The debate is not a transcript dump. The architect records the decision-relevant tensions, which persona won each disputed point, and the technical tradeoff accepted by the final TDD.

## Dependency Hop Mapping

The architect maps the design into Dependency Hops. Each hop states:

- The design decision or component.
- The technical assumption that must be true.
- The dependency, package, API, schema, platform capability, or operational condition required by that assumption.
- The evidence from the repo, documentation, tests, or local constraints that supports it.
- The failure mode if the assumption is false.
- The fallback, mitigation, or blocking question.

Use Dependency Hops to make hidden architecture assumptions reviewable before decomposition.

## Staged evaluator loop

Each TDD draft is reviewed by three fresh evaluator sub-agents in order. Register and add an `Agent receipts` row for each stage separately.

### Initial filter evaluator

Spawn a fresh evaluator with:

- Role: technical initial filter evaluator
- Ownership: read-only unless asked to produce an evaluator report inside the Doric run directory
- Inputs: architecture-scoped sections of `<run>/PROMPT.md`, `<run>/PRD.md`, current `<run>/TDD.md`
- Output: pass/fail on basic structure, deprecated libraries, invalid package versions, languages outside repository constraints, missing sections, and malformed assumptions
- Stop condition: TDD can proceed to grounded review or must be rejected for structural fixes

### Grounded evaluator

Spawn a fresh evaluator with:

- Role: technical grounded evaluator
- Ownership: read-only unless asked to produce an evaluator report inside the Doric run directory
- Inputs: architecture-scoped sections of `<run>/PROMPT.md`, `<run>/PRD.md`, current `<run>/TDD.md`, initial filter receipt row, relevant code context
- Output: findings against current architecture, package boundaries, data models, APIs, UI states, coding conventions, and test tooling
- Stop condition: TDD can proceed to assumption verification or a blocking architecture question is identified

### Assumption evaluator

Spawn a fresh evaluator with:

- Role: technical assumption evaluator
- Ownership: read-only unless asked to produce an evaluator report inside the Doric run directory
- Inputs: architecture-scoped sections of `<run>/PROMPT.md`, `<run>/PRD.md`, current `<run>/TDD.md`, grounded evaluator receipt row, relevant code context
- Output: independent verification of each Dependency Hop, including impossible assumptions and required rewrites
- Stop condition: TDD is ready for decomposition or a blocking technical assumption is identified

When any evaluator stage finds required architectural gaps, the coordinator writes or updates `<run>/GAPS_REPORT.md` and spawns a brand-new architect with only the current `<run>/TDD.md`, `<run>/GAPS_REPORT.md`, and the minimum required source artifacts. Do not let a stale architect thread keep revising from memory. Phase transition requires all evaluator stages in the accepted epoch to have accepted required-agent rows and receipt rows.

## GAPS_REPORT.md schema

```markdown
# Gaps Report

## Source evaluator receipts

## Blocking gaps

## Non-blocking gaps

## Invalid Dependency Hops

## Required rewrites

## Minimal context for next architect

## Coordinator notes
```

`GAPS_REPORT.md` is part of the technical-design loop. It records the latest accepted evaluator gaps and the minimal handoff package for the next stateless architect repair pass. If no gaps are found, `GAPS_REPORT.md` is optional.

## Technical tournament

Run a bounded pairwise tournament only when the architect or evaluator identifies multiple viable technical stacks, data models, API shapes, state models, or rollout strategies. Do not create alternatives just to run a tournament.

Tournament rules:

- Limit candidates to 2-4 serious alternatives.
- Spawn a fresh `technical tournament judge` evaluator for each pairwise comparison.
- Judge on correctness, compatibility with current architecture, performance, security, cost, migration risk, testability, and maintainability.
- Track simple Elo-style ratings in the tournament receipt row or directly in `TDD.md`.
- Select the highest-rated path unless it violates a hard PRD or repository constraint or creates a blocking Dependency Hop.

The final TDD records the selected path, rejected alternatives, rating summary, and the reason the winner is the best current technical bet.

## Technical design rubric

The evaluator stages collectively check:

- The design fits current architecture, package boundaries, and coding conventions.
- Generator persona tradeoffs are represented in final decisions.
- Data model changes are normalized enough for the domain and include migration risk.
- API contracts, event shapes, commands, or UI states are explicit.
- Dependency Hops are concrete, evidence-backed, and independently verified.
- Security boundaries, trust assumptions, authorization, secrets, and data exposure are considered.
- Performance, scalability, and operational failure modes are considered where relevant.
- Test seams are clear and compatible with existing test tooling.
- Tournament results are present when credible alternatives existed.
- Rollout, migration, or compatibility risks are named.

## TDD.md schema

```markdown
# Technical Design Document

## Summary

## Current architecture context

## Technical persona debate

## Proposed design

## Data model

## API or interface contracts

## Dependency Hops

## Technical alternatives and tournament

## Security and privacy

## Performance and operations

## Testing strategy

## Rollout and migration

## Risks

## Open questions
```

## Approval gate

Before decomposition, present the finalized `TDD.md` summary and request explicit user approval to proceed. Do not start decomposition or implementation without that approval.

When approval is granted, record:

- The approval text or a concise summary in a `STATE.md` `Agent receipts` row such as `approval_tdd_to_decomposition`
- `tdd_to_decomposition` as approved in the `Approvals` table in `<run>/STATE.md`
- `Phase: decomposition` in `<run>/STATE.md`

If approval is withheld or conditional, keep `Phase: technical_design` and record the requested changes before spawning another architect/evaluator pass.

## Completion checks

- `TDD.md` lives directly inside the Doric run directory.
- Architect and every evaluator-stage required-agent row and receipt row exists for every completed epoch, includes spawn proof, has accepted current rows before the phase advances, and leaves no active technical-design row pending, spawned, blocked, or rejected.
- `STATE.md` records the technical-design phase and has durable approval before decomposition begins.
- Every major PRD requirement has a technical path.
- Every major prompt architecture requirement is reflected in the design, deferred with a blocking question, or explicitly rejected as out of scope.
- Technical persona debate conclusions are captured without bloating the artifact with full transcripts.
- Dependency Hops map design decisions to assumptions, dependencies, evidence, failure modes, and fallbacks.
- If alternatives existed, the tournament receipt row or TDD records pairwise decisions and the winning path.
- If evaluator gaps existed, `<run>/GAPS_REPORT.md` records the gap handoff used by the fresh architect repair pass.
- Test seams and regression suites are identifiable.
- Risks are explicit enough for decomposition to plan incremental effort order.
