# Step 6: Handover

Use this reference for Doric completion reporting. Stop reading once the handover sub-agent prompt, schema, and completion checks are clear.

## Goal

Create `<run>/HANDOVER.md` only after all efforts are done and validated in the recorded execution order.

Do not begin this step unless `STATE.md` records `Phase: handover`, all efforts are `done`, every effort has a commit checkpoint, `Next effort index` equals the number of rows in the `Effort order` table, and there are no active ownership locks. If work is incomplete, stop in development and record the blocker in an agent receipt or validation record instead of creating final `HANDOVER.md`.

## Handover sub-agent

Spawn a fresh handover sub-agent with:

- Role: completion reporter
- Ownership: write or draft `<run>/HANDOVER.md`
- Inputs: `<run>/STATE.md`, `<run>/PROMPT.md`, `<run>/PRD.md`, `<run>/TDD.md`, `<run>/efforts/*.md`, `<run>/agents/*.md`, `<run>/validation/*.md`, test results, current diff or commit list
- Output: concise completion report
- Stop condition: the report summarizes completed work, verification, modified files, user decisions, residual risks, and follow-up work

Register the completion reporter in `STATE.md` `Required agents` before spawning it. The coordinator reviews the handover report against the artifacts and current diff, then marks the handover receipt and required-agent row `accepted` before presenting it to the user.

## HANDOVER.md schema

```markdown
# Handover

## Completed efforts

## Tests added or changed

## Test results

## Commit checkpoints

## Modified files

## User decisions

## Agent receipts

## Residual risks

## Follow-up work
```

## Completion checks

- `HANDOVER.md` lives directly inside the Doric run directory.
- All effort files are marked `done`.
- Completed efforts match the `Effort order` table in `STATE.md` from `01_` through `NN_`; no later effort is reported complete before an earlier effort.
- Test results include commands and outcomes.
- Validation records include red and green evidence for each implementation effort, or a documented validation-only exception.
- Commit checkpoints include one conventional commit subject per completed effort in effort order.
- Agent receipts exist for every required phase and effort role, include spawn proof, have accepted current rows in `STATE.md` `Required agents`, and leave no active required-agent row pending, spawned, blocked, or rejected.
- `STATE.md` has no active locks, `Current effort: none`, `Next effort index` equal to the effort count, and matches the final phase.
- Modified files match the actual diff or ordered commit list.
- Residual risks and follow-up work are concrete.
