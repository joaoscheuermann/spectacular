# Step 4: Decomposition

Use this reference for Doric feature extraction and effort planning. Stop reading once the extraction roles, validation rubric, and effort schema are clear.

## Goal

Create `<run>/FEATURES.md` plus small, ordered effort files under `<run>/efforts/` from `<run>/PRD.md` and `<run>/TDD.md`. The order is executable: development must run `01_` first, then `02_`, continuing one effort at a time through the final numbered effort.

Do not begin this step unless `<run>/STATE.md` records `tdd_to_decomposition` as approved in the `Approvals` table.

## Feature extraction sub-agent

Spawn a fresh feature extraction sub-agent with:

- Role: requirement extractor
- Ownership: write or draft `<run>/FEATURES.md`
- Inputs: `<run>/PROMPT.md`, `<run>/PRD.md`, `<run>/TDD.md`
- Output: complete feature and requirement list in `<run>/FEATURES.md`
- Stop condition: every PRD requirement and TDD component is represented

Register the requirement extractor in `STATE.md` `Required agents` before spawning it. Add a feature extraction row in `STATE.md` `Agent receipts` with spawn proof and a compact `Overview`, then mark the required-agent row `accepted` after coordinator review.

## Validator sub-agent

Spawn a separate validator sub-agent with:

- Role: decomposition validator
- Ownership: read-only
- Inputs: `<run>/FEATURES.md`, `<run>/PRD.md`, `<run>/TDD.md`
- Output: omissions, altered scope, duplicates, or approval to decompose
- Stop condition: feature list is faithful to source artifacts or blocking gaps are identified

Register the decomposition validator in `STATE.md` `Required agents` before spawning it. Add a decomposition validator row in `STATE.md` `Agent receipts` with spawn proof and a compact `Overview`, then mark the required-agent row `accepted` after coordinator review.

## Decomposition sub-agent

Spawn a decomposition sub-agent with:

- Role: effort planner
- Ownership: write or draft files under `<run>/efforts/`
- Inputs: validated `<run>/FEATURES.md`, `<run>/PRD.md`, `<run>/TDD.md`, relevant code/test context
- Output: one effort file per isolated implementation slice
- Stop condition: efforts are small, ordered, traceable, and testable

The decomposition sub-agent also drafts ownership boundaries for each effort. These are proposals until the coordinator records locks in the `Active locks` table in `STATE.md` during development.

Effort filenames must use contiguous two-digit numeric prefixes starting at `01_`. Do not emit gaps, duplicate prefixes, unnumbered files, or alternate ordering rules.

Register the effort planner in `STATE.md` `Required agents` before spawning it. Add an effort planner row in `STATE.md` `Agent receipts` with spawn proof, changed effort file paths, and the coordinator decision before requesting implementation approval.

## FEATURES.md schema

```markdown
# Features

## Source artifacts

## Extracted features

## Requirement coverage map

## Technical coverage map

## Assumption coverage

## Exclusions

## Validator notes
```

`FEATURES.md` is the durable bridge between `PRD.md`/`TDD.md` and effort planning. It must map every PRD requirement, User Value Hop, TDD component, and Dependency Hop to at least one feature or to an explicit exclusion.

## Decomposition validation rubric

The validator checks:

- Every PRD requirement and TDD component maps to at least one effort.
- Every extracted feature in `FEATURES.md` traces back to `PROMPT.md`, `PRD.md`, or `TDD.md`.
- Every User Value Hop and Dependency Hop is represented in the coverage map or explicitly excluded with a reason.
- No effort introduces scope outside the PRD/TDD.
- Efforts are small, independently reviewable, and ordered by dependency.
- Effort filenames form a contiguous `01_` to `NN_` sequence.
- Coupled files and required regression tests are listed.
- Acceptance criteria are specific enough for a test planner.
- No two efforts are marked `in-progress`.
- Each effort has a narrow enough target-file list for a worker to own without conflicting with other active workers.

## Effort file schema

```markdown
# Effort: <short action>

Status: todo

## Requirement links

## Goal

## Sequence

## Target files

## Coupled files

## Ownership

## Tests to add or update

## Regression suites

## Acceptance criteria

## Notes
```

The `Sequence` section must name the numeric position and immediate predecessor, for example `Position: 02 of 05` and `Previous effort: 01_workspace_identity.md`. The `Ownership` section must list the intended worker write scope, read-only context, and known conflict risks.

## Implementation approval gate

Before development, present the finalized effort list and request explicit user approval to start implementation. Record:

- The approval text or a concise summary in a `STATE.md` `Agent receipts` row such as `approval_decomposition_to_implementation`
- The ordered effort filenames in the `Effort order` table in `<run>/STATE.md`
- `Next effort index: 0` in `<run>/STATE.md`
- `decomposition_to_implementation` as approved in the `Approvals` table in `<run>/STATE.md`
- `Phase: development` in `<run>/STATE.md`

Do not spawn test-planning, test-writing, code-writing, validation, or review agents until this approval is recorded, every current decomposition role has an accepted required-agent row, and no active decomposition row is pending, spawned, blocked, or rejected.

## Completion checks

- Effort files live under `<run>/efforts/`.
- `FEATURES.md` lives directly inside the Doric run directory.
- `FEATURES.md` maps source requirements, technical components, User Value Hops, and Dependency Hops to extracted features before effort files are planned.
- Effort filenames are contiguous and ordered from `01_` to `NN_`.
- Each effort has a `todo`, `in-progress`, or `done` status.
- Every new effort starts with `Status: todo`, and the matching `STATE.md` `Effort order` row also starts as `todo`.
- Each effort includes a `Sequence` section matching the filename order.
- Each effort includes graph-based impact analysis through target files, coupled files, and regression suites.
- Each effort includes explicit ownership boundaries.
- Efforts can be implemented one at a time through a focused red-green-refactor cycle.
- Requirement extractor, decomposition validator, and effort planner required-agent rows and receipt rows exist, include spawn proof, have accepted current rows, and leave no active decomposition row pending, spawned, blocked, or rejected.
- `STATE.md` records the same ordered effort list, `Next effort index: 0`, decomposition completion, and implementation approval before development starts.
