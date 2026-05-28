# Step 3: Technical Design

Use this reference for the Doric architecture and technical design phase. Stop reading once the architect/evaluator loop, design rubric, schema, and approval gate are clear.

## Goal

Create `<run>/TDD.md` from `<run>/PROMPT.md`, `<run>/PRD.md`, current repo architecture, and relevant coding standards. Treat `TDD.md` as a Technical Design Document, not the development method.

## Default loop

Run up to 3 architect/evaluator epochs unless the user gives a different limit. Ask the user for missing architectural constraints only when they affect irreversible decisions such as data stores, API style, deployment model, compatibility, or required libraries.

Register each architect and evaluator pass in `STATE.md` `Required agents` before spawning it. Record each architect and evaluator pass as a spawn-backed receipt under `<run>/agents/`. The coordinator updates `STATE.md` only after reviewing the evaluator result and marking the accepted required-agent rows.

## Architect sub-agent

Spawn a fresh architect sub-agent with:

- Role: technical design author
- Ownership: write or draft `<run>/TDD.md`
- Inputs: `<run>/PROMPT.md`, `<run>/PRD.md`, current repo architecture, relevant standards
- Output: complete technical design draft
- Stop condition: design satisfies the schema or names blocking architecture facts

When the evaluator reports gaps, spawn a fresh architect with the current `<run>/TDD.md` and evaluator report rather than letting a stale thread continue from memory.

## Evaluator sub-agent

Spawn a separate evaluator sub-agent with:

- Role: technical design evaluator
- Ownership: read-only unless asked to produce an evaluator report inside the Doric run directory
- Inputs: `<run>/PROMPT.md`, `<run>/PRD.md`, current `<run>/TDD.md`, relevant code context
- Output: prioritized findings and required edits
- Stop condition: design is ready for decomposition or a blocking architecture question is identified

## Technical design rubric

The evaluator checks:

- The design fits current architecture, package boundaries, and coding conventions.
- Data model changes are normalized enough for the domain and include migration risk.
- API contracts, event shapes, commands, or UI states are explicit.
- Security boundaries, trust assumptions, authorization, secrets, and data exposure are considered.
- Performance, scalability, and operational failure modes are considered where relevant.
- Test seams are clear and compatible with existing test tooling.
- Rollout, migration, or compatibility risks are named.

## TDD.md schema

```markdown
# Technical Design Document

## Summary

## Current architecture context

## Proposed design

## Data model

## API or interface contracts

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

- The approval text or a concise summary in `<run>/agents/<next>_approval_tdd_to_decomposition.md`
- `tdd_to_decomposition` as approved in the `Approvals` table in `<run>/STATE.md`
- `Phase: decomposition` in `<run>/STATE.md`

If approval is withheld or conditional, keep `Phase: technical_design` and record the requested changes before spawning another architect/evaluator pass.

## Completion checks

- `TDD.md` lives directly inside the Doric run directory.
- Architect and evaluator required-agent rows and receipts exist for every completed epoch, include spawn proof, have accepted current rows before the phase advances, and leave no active technical-design row pending, spawned, blocked, or rejected.
- `STATE.md` records the technical-design phase and has durable approval before decomposition begins.
- Every major PRD requirement has a technical path.
- Test seams and regression suites are identifiable.
- Risks are explicit enough for decomposition to plan incremental effort order.
