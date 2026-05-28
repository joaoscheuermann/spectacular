# Step 2: PRD Generation

Use this reference for the Doric product requirements phase. Stop reading once the PM/evaluator loop, PRD rubric, and `PRD.md` schema are clear.

## Goal

Create `<run>/PRD.md` from `<run>/PROMPT.md` and user answers.

## Default loop

Run up to 3 PM/evaluator epochs unless the user gives a different limit. Pause for user clarification when the evaluator finds a structural gap that requires domain knowledge.

Register each PM and evaluator pass in `STATE.md` `Required agents` before spawning it. Record each PM and evaluator pass as a spawn-backed receipt under `<run>/agents/`. The coordinator updates `STATE.md` only after reviewing the evaluator result and marking the accepted required-agent rows.

## Product manager sub-agent

Spawn a fresh product manager sub-agent with:

- Role: PRD writer
- Ownership: write or draft `<run>/PRD.md`
- Inputs: `<run>/PROMPT.md`, user answers, relevant product constraints
- Output: complete PRD draft
- Stop condition: PRD draft satisfies the schema or names the missing facts

## Evaluator sub-agent

Spawn a separate evaluator sub-agent with:

- Role: PRD evaluator
- Ownership: read-only unless asked to produce an evaluator report inside the Doric run directory
- Inputs: `<run>/PROMPT.md`, current `<run>/PRD.md`
- Output: prioritized findings and required edits
- Stop condition: PRD is ready for technical design or a blocking product question is identified

When an evaluator finds required edits, spawn a fresh product manager sub-agent with the current `<run>/PRD.md` and evaluator receipt. Do not let the same draft thread keep revising from memory.

## PRD rubric

The evaluator checks:

- Requirements are concrete, testable, and unambiguous.
- Personas and primary workflows are explicit.
- Acceptance criteria cover success, failure, empty, and edge states.
- Out-of-scope boundaries prevent speculative implementation.
- KPIs or success measures are measurable.
- Compliance, privacy, safety, accessibility, or operational concerns are named when relevant.
- Open questions are separated from resolved decisions.

## PRD.md schema

```markdown
# Product Requirements Document

## Problem statement

## Goals

## Non-goals

## Personas

## Primary workflows

## Functional requirements

## Acceptance criteria

## Success measures

## Risks and compliance

## Open questions
```

## Completion checks

- `PRD.md` lives directly inside the Doric run directory.
- PM and evaluator required-agent rows and receipts exist for every completed epoch, include spawn proof, have accepted current rows before the phase advances, and leave no active PRD row pending, spawned, blocked, or rejected.
- `STATE.md` records `Phase: prd` while PRD generation is active and advances only after evaluator approval or documented non-blocking gaps.
- Every goal traces back to `PROMPT.md`.
- Non-goals are strong enough to constrain decomposition.
- Acceptance criteria are specific enough for a test planner.
- Remaining open questions do not block technical design.
