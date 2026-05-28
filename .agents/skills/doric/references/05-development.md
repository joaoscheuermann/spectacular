# Step 5: Development

Use this reference for Doric per-effort implementation. Stop reading once the red-green-refactor roles, circuit breaker, review rubric, and status rules are clear.

## Goal

Implement one `<run>/efforts/*.md` file at a time with separate test, code, validation, and review sub-agents. Efforts must run strictly in the `Effort order` table in `<run>/STATE.md`, from index `0` through the final index.

Do not begin this step unless `<run>/STATE.md` records `decomposition_to_implementation` as approved and has a non-empty `Effort order` table. For the first effort, `Next effort index` must be `0`. When resuming, `Next effort index` must point to the earliest effort that is not already `done`. Resume by reading `STATE.md`, effort statuses, current locks, validation records, and worktree status.

## Per-effort sequence

For the next legal effort:

1. Resolve the next legal effort as the row in `STATE.md` where `Index` equals `Next effort index`. Do not choose a different `todo` effort.
2. Verify no other effort is `in-progress`. If another effort is active, resume or close that effort before starting a new one.
3. Verify every earlier effort in the `Effort order` table is marked `done`, has green validation evidence, has reviewer approval, and has no active locks.
4. Run the `todo -> in-progress` status transition protocol.
5. Register and spawn a test-planner sub-agent to translate acceptance criteria into concrete tests and commands. Store its spawn-backed receipt under `<run>/agents/` and mark the required-agent row `accepted` after coordinator review.
6. Register and spawn a test-writer sub-agent that owns only test files and does not write implementation code. The worker must add or update tests, run the focused command, capture expected failing output before implementation starts, store its spawn-backed receipt, and be marked `accepted`.
7. Record red evidence in `<run>/validation/<effort_name>.md`: command, exit code, failure summary, and why the failure maps to the effort acceptance criteria.
8. Register and spawn a code-writer sub-agent that owns implementation files and makes the recorded tests pass while following the coding-conventions gate below. Store its spawn-backed receipt and mark it `accepted` only after reviewing the changed files.
9. Register and spawn a validator/refactor sub-agent to run focused tests, inspect quality, and recommend or apply narrow refactors within assigned ownership. Store its spawn-backed receipt and mark it `accepted` only after green evidence is recorded.
10. Record green evidence in `<run>/validation/<effort_name>.md`: focused commands, regression commands, exit codes, unavailable tooling, and output summaries.
11. Register and spawn a reviewer sub-agent after tests pass to compare the diff against the effort scope. Store its spawn-backed receipt and mark it `accepted` only after the reviewer approves or all required fixes are completed by fresh sub-agents.
12. Run the `in-progress -> done` status transition and commit checkpoint protocols only after focused tests, relevant regression tests, scope review, and all required development-agent rows for the current effort are accepted.

Repeat this sequence until `Next effort index` equals the number of rows in the `Effort order` table. Only then may the coordinator move to handover.

## Required development-agent gate

Each effort must have one accepted `STATE.md` `Required agents` row for each role before it can be marked `done`:

- test planner
- test writer
- code writer
- validator/refactor
- reviewer

Each row must use `Phase: development`, the exact effort filename, the expected agent type from the Doric role matrix, the receipt path, the returned agent id, and `Status: accepted`. If any role is missing, pending, spawned, blocked, rejected, or lacks spawn proof, leave the effort `in-progress`, keep `Next effort index` unchanged, and record the blocker.

## Coding conventions gate

Every development worker that touches or approves tests or implementation must follow `.agents/skills/coding-conventions/SKILL.md`.

The coordinator includes that skill path in the prompts for:

- test-planning sub-agents
- test-writer sub-agents
- code-writer sub-agents
- validator/refactor sub-agents
- reviewer sub-agents

Each worker must:

- Load `.agents/skills/coding-conventions/SKILL.md` before making recommendations, edits, validation decisions, or review approvals.
- Determine whether the effort touches architecture, implementation, testing, scaffolding, Rust, TypeScript/JavaScript, Python, or mixed-language boundaries.
- Read only the coding-conventions reference files that match the effort's language and task.
- Apply repository invariants for simplicity, deep modules, dependency boundaries, naming, file-size limits, and test placement.
- Apply language-specific guidance only to matching files.
- Record in its agent receipt which coding-conventions references it used and how they constrained the work.

If the coding-conventions skill is missing, unreadable, or materially ambiguous for the assigned effort, the worker stops and reports a blocker instead of editing or approving code.

## Status transition protocol

Status updates are atomic workflow gates. The coordinator must update both the effort file and `STATE.md` before spawning the next sub-agent or moving to the next effort.

### `todo -> in-progress`

Before spawning the test-planning sub-agent:

1. Read the target effort file and the matching `STATE.md` `Effort order` row.
2. Confirm both statuses are `todo`.
3. If the statuses disagree, stop and write a reconciliation receipt under `<run>/agents/`; do not spawn workers.
4. Update the effort file header to `Status: in-progress`.
5. Update the matching `STATE.md` `Effort order` row to `in-progress`.
6. Set `Current effort` in `STATE.md` to the effort filename.
7. Add active locks for the assigned write scope in `STATE.md`.
8. Write or update a coordinator transition receipt documenting the transition.

### `in-progress -> done`

Before resolving the next legal effort:

1. Confirm the effort file and matching `STATE.md` row are both `in-progress`.
2. Confirm red evidence, green evidence, reviewer approval, accepted required-agent rows for the current effort, and no unresolved ownership conflicts.
3. Update the effort file header to `Status: done`.
4. Update the matching `STATE.md` `Effort order` row to `done`.
5. Update the `Validation records` table in `STATE.md`.
6. Prepare final `STATE.md` completion updates: release the effort's active locks, set `Current effort` to `none`, increment `Next effort index` by one, and update the `Commit checkpoints` table with the planned conventional commit subject.
7. Run the commit checkpoint protocol.
8. If the commit succeeds, update the transition receipt with the commit subject and hash when available.
9. If staging or commit fails, restore the effort file and `STATE.md` to the pre-completion `in-progress` state, keep `Current effort` and locks visible, record the blocker, and do not increment `Next effort index`.

If validation fails, review fails, a circuit breaker is reached, or the commit checkpoint fails, leave both statuses as `in-progress`, keep `Current effort` set, keep unresolved locks visible, and record the blocker. Do not increment `Next effort index`.

## Commit checkpoint protocol

Each completed effort creates one conventional Git commit before the next effort can start.

Before committing:

1. Inspect `git status` and identify unrelated dirty changes.
2. Stage only the current effort's approved files and required Doric artifacts: the effort file, `STATE.md`, relevant agent receipts, and the validation record.
3. Leave unrelated user or worker changes unstaged.
4. Confirm the staged scope matches the effort ownership, coupled files, and validation artifacts.
5. Commit with a conventional commit message.
6. Record the commit subject and commit hash when available in the transition receipt, validation record, and `STATE.md` `Commit checkpoints` table.

Conventional commit format:

```text
<type>(<scope>): <summary>
```

Type defaults:

- `feat` for user-visible capability
- `fix` for bug or broken behavior
- `refactor` for internal restructuring without behavior change
- `test` for test-only efforts
- `docs` for documentation-only efforts
- `chore` for metadata, validation, tooling, or Doric-only efforts

Use a lowercase kebab-case scope from the primary package, module, or effort name. Write the summary in imperative mood from the effort goal. Add a breaking-change marker or footer only when the effort intentionally introduces a breaking change.

If `git status`, staging, or commit fails, stop at the current effort. Do not mark the effort complete, release locks, or start the next effort until the failure is resolved.

## Worker prompt invariant

For workers that edit code, include: "You are not alone in the codebase. Do not revert edits made by others. Keep changes within your assigned files unless you discover a blocking conflict and report it."

Code-edit workers must return changed paths and any files they read outside their write ownership. If they need to touch a file outside the assigned scope, they stop and report the conflict instead of widening the change silently.

For code-edit workers, also include: "Load and follow `.agents/skills/coding-conventions/SKILL.md` before editing. Read only the convention references that match this effort's language and task. Record the references used in your receipt."

## Validation record schema

```markdown
# Validation: <effort name>

## Red evidence

## Green evidence

## Focused commands

## Regression commands

## Unavailable tooling

## Refactors applied

## Reviewer decision
```

Red evidence is mandatory unless the effort is explicitly validation-only. For validation-only efforts, record why a red test is not applicable and what baseline evidence replaces it.

## Circuit breaker

Default to 5 implementation/validation attempts per effort. If tests still fail, stop and escalate with:

- Current effort path
- Current `Next effort index` and ordered effort list
- Focused test command and failure output
- Current diff summary
- Attempts already made
- Commit checkpoint state if staging or commit was attempted
- Smallest next user decision needed

Do not start the next effort after a circuit breaker event. Development remains stopped at the current effort until the user approves a fix, re-decomposition, or an explicit artifact update that changes the ordered effort list.

## Development review rubric

The reviewer checks:

- The relevant development workers loaded `.agents/skills/coding-conventions/SKILL.md` and recorded the applied convention references.
- `STATE.md` has accepted required-agent rows for the effort's test planner, test writer, code writer, validator/refactor, and reviewer.
- Each development receipt includes spawn proof, the expected agent type, the returned agent id, and a coordinator decision of `accepted`.
- Tests and implementation follow the coding-conventions repository invariants and the applicable language-specific guidance.
- Red evidence was captured before implementation, or the validation-only exception is justified.
- Tests exercise public behavior and match the effort acceptance criteria.
- Implementation is scoped to the effort and does not include unrelated refactors.
- Existing conventions, naming, and architecture boundaries are preserved.
- Focused tests and impact-analysis regression tests passed.
- A conventional commit checkpoint exists for the effort and stages only effort-owned changes plus required Doric artifacts.
- The effort file status is updated only after validation passes.
- The effort file status and the matching `STATE.md` `Effort order` row status agree.
- The `todo -> in-progress` transition receipt exists before test-planning.
- The `in-progress -> done` transition receipt exists before `Next effort index` advances and records the commit subject plus hash when available.
- `STATE.md` locks and validation records match the actual changed files.
- The effort reviewed is exactly the effort captured in `Current effort` when the effort started.
- Earlier efforts in the `Effort order` table are all complete before this effort is marked `done`.

## Commit and rollback

- Commit after every green effort with a conventional commit message before starting the next effort.
- Prefer non-destructive recovery: stop, report the failing effort, and preserve evidence.
- Use destructive rollback commands only with explicit user approval and only after verifying the target paths and current status.
