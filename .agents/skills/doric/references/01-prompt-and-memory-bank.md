# Step 1: Prompt and Memory Bank

Use this reference for the Doric requirements elicitation phase. Stop reading once the requirements sub-agent prompt and `PROMPT.md` schema are clear.

## Goal

Create the durable source of truth for the feature run at `<run>/PROMPT.md`, where `<run>` is the active `.doric/<MM-YYYY>/<DD>/<timestamp>-<feature_name>/` directory.

Before spawning the requirements sub-agent, the coordinator initializes `<run>/STATE.md` with `Phase: prompt`, `Current effort: none`, `Next effort index: 0`, and empty `Effort order`, `Active locks`, `Required agents`, `Agent receipts`, `Validation records`, and `Commit checkpoints` tables.

## Requirements sub-agent

Spawn a fresh requirements sub-agent with:

- Role: requirements elicitor
- Ownership: read-only unless the coordinator explicitly asks it to draft `PROMPT.md`
- Inputs: user request, existing user answers, relevant existing Doric artifacts if continuing a run
- Output: structured requirements summary or blocking clarification questions
- Stop condition: enough information exists to write `PROMPT.md`, or the missing facts would materially change the feature definition

Ask concise questions when required. Prefer questions about user value, business driver, personas, constraints, ambiguity, and non-goals over implementation details. Sub-agents return questions to the coordinator; the coordinator asks the user, records answers under resolved decisions or open questions, and then resumes the workflow.

Register the requirements elicitor in `STATE.md` `Required agents` before spawning it. Write a requirements receipt under `<run>/agents/` with spawn proof, then mark the required-agent row `accepted` before moving to PRD generation.

## PROMPT.md schema

```markdown
# Prompt

## Feature summary

## User value

## Business or commercial driver

## Personas

## Constraints

## Resolved decisions

## Open questions

## Non-goals
```

## Completion checks

- `PROMPT.md` lives directly inside the Doric run directory.
- `STATE.md` exists and records `Phase: prompt` until the coordinator advances it.
- A requirements agent receipt exists under `<run>/agents/`, includes spawn proof, is marked `accepted` in both the receipt and `STATE.md` `Required agents`, and no active prompt required-agent row is pending, spawned, blocked, or rejected.
- The feature summary is specific enough to name the work.
- Resolved decisions and open questions are separate.
- Non-goals constrain later PRD and implementation scope.
- Open questions do not block PRD generation unless they would materially change requirements.
