---
name: doric
description: Coordinate feature delivery through a sub-agent-driven workflow named Doric, from requirements elicitation through PRD, technical design, decomposition, test-driven implementation, validation, and handover. Use when the user asks Codex to run feature planning or development as sub-agents, create Doric artifacts, or manage a multi-agent product, architecture, and development loop.
---

# Doric

Use this skill to turn a feature idea into implementation through a controlled sequence of sub-agent phases. The coordinator agent owns state, user gates, worktree safety, artifact storage, and final integration; each major phase is delegated to a fresh, narrow sub-agent unless the phase explicitly needs a bounded generator/evaluator loop.

For detailed evaluator rubrics, artifact schemas, handoff prompts, and default loop limits, read only the reference for the current workflow step.

## Success criteria

- **Doric artifact root:** Every run stores and reads workflow artifacts under `.doric/<MM-YYYY>/<DD>/<timestamp>-<feature_name>/`. Use local date/time. Use a filesystem-safe timestamp such as `YYYYMMDD-HHMMSS` and a lowercase snake_case `feature_name`.
- **Source of truth:** The run directory contains durable artifacts that prevent context drift: `STATE.md`, `PROMPT.md`, `PRD.md`, `GAPS_REPORT_PRD.md` when product gaps are found, `TDD.md`, `FEATURES.md`, `GAPS_REPORT.md` when technical gaps are found, `logs/`, `efforts/*.md`, required-agent ledger entries, validation records, commit checkpoints, and a handover report.
- **Sub-agent execution:** Requirements, PRD drafting, staged PRD evaluation, architecture drafting, staged architecture evaluation, decomposition, test planning, test writing, implementation, validation/refactor, final review, and handover are each handled by dedicated sub-agents with narrow prompts and spawn-backed receipt rows in `STATE.md`.
- **Reasoning artifacts:** PRD and technical-design loops persist persona debate summaries, assumption-hop maps, staged evaluator findings, and bounded pairwise tournament decisions when competing product or architecture paths exist.
- **Human gates:** The coordinator stops after `PROMPT.md`, enters prompt-to-PRD alignment mode, raises every open question with the user, and records `prompt_to_prd_alignment` in `STATE.md` before PRD generation. The coordinator also records explicit approvals before moving from `TDD.md` into decomposition or from validated decomposition into implementation.
- **Implementation discipline:** Development processes efforts strictly in numeric filename order from `01_` to `NN_`, one effort at a time. Each effort follows a red-green-refactor loop, records red and green evidence, keeps edits scoped, preserves unrelated worktree changes, and runs the tests identified by impact analysis.
- **Traceability:** Every effort file lists status, target files, acceptance criteria, tests to run, and links back to the PRD/TDD requirements it satisfies.
- **Receipts:** Every spawned sub-agent has a durable row in `STATE.md` `Agent receipts`, every required sub-agent is registered in `STATE.md`, every validation pass has a durable record under `<run>/validation/`, and every completed effort has a conventional commit checkpoint. Standalone files under `<run>/agents/` are optional detail for large outputs, blocked/rejected work, or review artifacts.

## Retrieval and stop rules

- Start with the user request and the most relevant existing run under `.doric/`. If the user names a feature, find the matching `.doric/<MM-YYYY>/<DD>/<timestamp>-<feature_name>/` directory. If multiple runs match, use the newest run unless the user directs otherwise.
- For a new feature run, create the run directory and initialize `STATE.md` before writing any workflow artifact. Store all generated artifacts inside that run directory.
- For an existing run, read `STATE.md`, effort statuses, latest `Agent receipts` rows, validation records, and current worktree status before choosing the next legal phase. If the state is missing or contradictory, reconstruct the smallest safe state from artifacts and record the reconciliation in `STATE.md`.
- Read only the current step reference when drafting prompts for phase-specific sub-agents, validating artifacts, or checking required fields.
- Read repo architecture, coding standards, test docs, and nearby code only when they are needed to answer a specific phase question.
- Stop reading once the current sub-agent has enough evidence to produce its artifact or identify a blocking question.
- Pass raw artifacts from the Doric run directory to evaluator and reviewer sub-agents. Do not pass the coordinator's private conclusions, expected answer, or intended fix unless the review explicitly depends on that context.
- If sub-agent tools are unavailable, stop and tell the user which phase cannot be delegated instead of silently collapsing the workflow into a single-agent run.

## Coordinator responsibilities

- Begin with a short preamble naming the current phase and the Doric run directory.
- Maintain `STATE.md` as the phase ledger. Update it after each phase transition, approval, sub-agent receipt row, ownership lock, validation command, and effort status change.
- Keep the worktree safe: inspect status before implementation, preserve unrelated changes, and avoid destructive rollback unless the user explicitly approves it.
- Spawn fresh sub-agents for independent phases and for evaluator passes. Reuse a sub-agent only inside a bounded loop where continuity is required.
- Register every required sub-agent in the `Required agents` table in `STATE.md` before spawning it, then update the same row after spawn and after coordinator review.
- Do not perform required sub-agent work locally in the coordinator. The coordinator may prepare prompts, update state, merge artifacts, ask user questions, and accept or reject results, but a required role only counts when a spawned sub-agent produces it.
- Give worker sub-agents disjoint write ownership, tell them they are not alone in the codebase, and instruct them not to revert edits made by others.
- Give each sub-agent a role, phase goal, read/write ownership, input artifact paths inside the Doric run directory, output path or response shape, applicable rubric, and stop condition.
- Review every sub-agent result before using it as input to the next phase.
- Write or update an `Agent receipts` row for every sub-agent result before advancing the workflow, even when the sub-agent only reports blockers.
- Ask the user only through the coordinator. Sub-agents return blocking questions to the coordinator; the coordinator asks the user, records the answer in the relevant artifact, and resumes delegation.

## Run state

Initialize `<run>/STATE.md` with this shape and keep it current:

```markdown
# State

## Cursor

- Phase: prompt
- Current effort: none
- Next effort index: 0

## Approvals

| Gate                            | Approved | Evidence |
| ------------------------------- | -------- | -------- |
| prompt_to_prd_alignment         | no       |          |
| tdd_to_decomposition            | no       |          |
| decomposition_to_implementation | no       |          |

## Effort order

| Index | Effort file | Status |
| ----- | ----------- | ------ |

## Active locks

| Effort | Owner | Write scope | Status |
| ------ | ----- | ----------- | ------ |

## Required agents

| Phase | Effort | Role | Agent type | Receipt | Agent id | Status |
| ----- | ------ | ---- | ---------- | ------- | -------- | ------ |

## Agent receipts

| Receipt | Phase | Effort | Role | Agent type | Agent id | Status | Overview |
| ------- | ----- | ------ | ---- | ---------- | -------- | ------ | -------- |

## Validation records

| Effort | Record | Red | Green | Reviewer |
| ------ | ------ | --- | ----- | -------- |

## Commit checkpoints

| Effort | Commit | Message | Staged scope |
| ------ | ------ | ------- | ------------ |
```

Use these legal phase transitions:

```text
prompt -> prd: PROMPT.md completion checks pass, every prompt role has an accepted required-agent row, no active prompt row is pending, spawned, blocked, or rejected, every PROMPT.md open question has been raised to the user, and prompt_to_prd_alignment approval is true
prd -> technical_design: the PRD phase orchestrator has an accepted required-agent row and receipt, the receipt proves a zero-gap current champion, accepted PRD evolution child evidence, and no active PRD phase-orchestrator row or child PRD role is pending, spawned, blocked, or rejected
technical_design -> decomposition: staged TDD evaluators approve, every technical-design role for completed epochs has an accepted required-agent row, no active technical-design row is pending, spawned, blocked, or rejected, and tdd_to_decomposition approval is true
decomposition -> development: FEATURES.md and effort files validate, every decomposition role has an accepted required-agent row, no active decomposition row is pending, spawned, blocked, or rejected, the Effort order table is recorded, and decomposition_to_implementation approval is true
development -> handover: all efforts in the Effort order table are done, every effort has accepted development required-agent rows, no active development row is pending, spawned, blocked, or rejected, every effort has a commit checkpoint, Next effort index equals the effort count, and no active locks remain
handover -> complete: HANDOVER.md passes completion checks, every handover role has an accepted required-agent row, and no active handover row is pending, spawned, blocked, or rejected
```

Do not skip phases. If resuming a run where artifacts are ahead of `STATE.md`, reconcile the state first and record the evidence.

## Effort execution order

Efforts execute strictly in filename order from `01_...md` through `NN_...md`.

- The decomposition phase must create effort files with contiguous two-digit numeric prefixes: `01_`, `02_`, `03_`, and so on. Gaps, duplicates, and unnumbered effort files are invalid.
- Before development starts, write the ordered effort filenames to the `Effort order` table in `STATE.md` and set `Next effort index` to `0`.
- The only legal effort to start is the row in `STATE.md` whose `Index` equals `Next effort index`.
- Do not start a later effort while an earlier effort is `todo`, `in-progress`, missing validation evidence, or missing reviewer approval.
- When an effort is marked `done`, require accepted development-agent rows for that effort, append or record its validation evidence, stage and commit its approved changes with a conventional commit message, clear `Current effort`, release its locks, increment `Next effort index` by one, and only then consider the next effort.
- If an effort hits the circuit breaker or blocks on a conflict, stop the development phase at that effort. Do not skip forward. Resume only after the user approves a fix, re-decomposition, or an explicit artifact update that changes the ordered effort list.

Parallel or disjoint worker ownership never authorizes multiple efforts to run at the same time. It only limits file ownership inside the currently active effort.

## Effort status protocol

Effort status exists in two places and both must match:

- The effort file header: `Status: todo`, `Status: in-progress`, or `Status: done`
- The matching row in the `Effort order` table in `STATE.md`

Before starting an effort:

- Compare the effort file status with the `STATE.md` row status.
- If they disagree, stop and reconcile before spawning any sub-agent.
- The only legal starting status is `todo`.
- Change both statuses to `in-progress`.
- Set `Current effort` to the effort filename.
- Add active locks for the assigned write scope.
- Write or update a coordinator transition note in `STATE.md` recording the status transition.
- Spawn test-planning only after those updates are complete.

Before moving to the next effort:

- Require red evidence, green evidence, reviewer approval, accepted required-agent rows for the effort, and no unresolved lock conflicts.
- Change both statuses to `done`.
- Update the `Validation records` table for the effort.
- Stage and commit the effort's approved changes with a conventional commit message.
- Update the `Commit checkpoints` table for the effort.
- Release active locks.
- Clear `Current effort`.
- Increment `Next effort index` by one.
- Write or update a coordinator transition note in `STATE.md` recording the transition.
- Resolve the next legal effort only after those updates are complete.

If a circuit breaker, failed validation, or ownership conflict stops the effort, leave both statuses as `in-progress`, keep `Current effort` set, and record the blocker in an `Agent receipts` row or validation record. Do not set `done`, release locks, or increment `Next effort index`.

If staging or committing fails, restore the effort status and `STATE.md` cursor to the pre-completion `in-progress` state, keep locks visible, record the blocker, and do not resolve the next effort.

Coordinator transition notes document state changes only. They do not satisfy required-agent gates and should not be listed as required-agent rows unless a spawned sub-agent actually performed that role.

## Commit checkpoint protocol

Each completed effort must create exactly one Git commit before the coordinator can start the next effort.

- Inspect `git status` before staging.
- Stage only files owned by the current effort plus its Doric artifacts: the effort file, `STATE.md`, optional detailed agent files when used, and the validation record.
- Preserve unrelated user or worker changes by leaving them unstaged.
- Use a conventional commit message: `<type>(<scope>): <summary>`.
- Choose the type from the effort's primary change: `feat`, `fix`, `refactor`, `test`, `docs`, or `chore`.
- Derive the scope from the primary package, module, or effort name using lowercase kebab-case.
- Write the summary in imperative mood and derive it from the effort goal.
- Include a breaking-change marker or footer only when the effort intentionally introduces a breaking change.
- Record the commit subject and commit hash when available in the transition note and `Commit checkpoints` table.
- If staging or commit fails, stop at the current effort and keep `Next effort index` unchanged.

## Sub-agent enforcement protocol

Doric treats sub-agent execution as a hard workflow gate, not an implementation preference.

- Before spawning a required role, add a row to `STATE.md` `Required agents` with the phase, effort or `none`, role, planned agent type, planned receipt id, blank agent id, and `Status: pending`.
- Spawn the sub-agent with `multi_agent_v1.spawn_agent`. After the tool returns, update the required-agent row with the returned agent id or nickname, the receipt id, and `Status: spawned`.
- If the spawn tool is unavailable, errors, or returns no usable agent identity, update the required-agent row to `Status: blocked`, add an `Agent receipts` row whose `Overview` states the failure, stop the workflow, and tell the user which phase or effort role could not be delegated.
- The coordinator may not replace a required role by writing the output locally. Local coordinator synthesis is allowed only for state reconciliation, user-facing summaries, prompt assembly, receipt review, and integrating accepted sub-agent artifacts.
- A required-agent row counts as complete only when the matching `Agent receipts` row exists, the returned agent id is recorded as spawn proof, the coordinator has reviewed the output, and both the receipt row and `Required agents` row say `Status: accepted`.
- Use only these required-agent statuses: `pending`, `spawned`, `accepted`, `blocked`, `rejected`, and `superseded`.
- If the coordinator rejects a sub-agent output, set the required-agent row to `Status: rejected`, record the reason in the `Agent receipts` row `Overview`, and spawn a fresh replacement role before advancing. After the replacement is accepted, mark the rejected row `superseded` and name the replacement receipt id in the rejected row's `Overview`.
- Phase transitions are illegal while any active required-agent row for that phase or current effort is `pending`, `spawned`, `blocked`, or `rejected`. Superseded rows are historical evidence and do not satisfy the gate.

Use this default role-to-agent-type mapping unless a reference file gives a stricter instruction:

| Phase | Required role | Agent type |
| ----- | ------------- | ---------- |
| prompt | requirements elicitor | explorer, or worker if drafting `PROMPT.md` |
| prd | PRD phase orchestrator | worker |
| prd child | PM security/compliance advocate | worker |
| prd child | PM lean-UX advocate | worker |
| prd child | PM scale/performance advocate | worker |
| prd child | proximity filter | explorer |
| prd child | tournament evaluator | explorer |
| prd child | evolution PM | worker |
| prd child | gap evaluator | explorer |
| prd child | mutation PM | worker |
| prd child | championship evaluator | explorer |
| technical_design | technical design author | worker |
| technical_design | technical initial filter evaluator | explorer |
| technical_design | technical grounded evaluator | explorer |
| technical_design | technical assumption evaluator | explorer |
| technical_design | technical tournament judge | explorer |
| decomposition | requirement extractor | explorer |
| decomposition | decomposition validator | explorer |
| decomposition | effort planner | worker |
| development | test planner | worker |
| development | test writer | worker |
| development | code writer | worker |
| development | validator/refactor | worker |
| development | reviewer | explorer, or worker if writing a review artifact |
| handover | completion reporter | worker |

## Sub-agent contract

Use the available sub-agent spawn tool for every delegated phase. In this environment, that is `multi_agent_v1.spawn_agent`.

- Use `agent_type: "explorer"` for read-only codebase questions and artifact audits.
- Use `agent_type: "worker"` for code, test, artifact, or validation work with a bounded write scope.
- Leave `model` unset unless the user requested a specific model or a task clearly needs an override.
- Do not use `fork_context` for Doric sub-agents. Pass explicit artifact paths, recorded user decisions, relevant receipt ids, and narrow role instructions instead. If a sub-agent appears to need hidden coordinator context, stop and write the missing fact into the appropriate run artifact or `STATE.md` before spawning.
- For code-edit workers, assign disjoint write ownership and require the invariant: "You are not alone in the codebase. Do not revert edits made by others. Keep changes within your assigned files unless you discover a blocking conflict and report it."
- For implementation-stage workers that plan tests, write tests, write code, validate/refactor, or review code, include `.agents/skills/coding-conventions/SKILL.md` as a required input artifact. The worker must load that skill, read only the language/task-specific references it requires, and follow those conventions before editing or approving code.

Every spawned sub-agent must have a matching `STATE.md` `Agent receipts` row. Use a compact receipt id such as `prd_evolution_orchestrator_01`, `05_code_writer_01`, or `handover_reporter`. The `Overview` column must be a short reason why the agent succeeded, failed, blocked, or was rejected. Include the key output path, changed files, blocking question, or replacement receipt id when relevant.

Only `accepted` `Agent receipts` rows satisfy required-agent ledger gates. Create a standalone file under `<run>/agents/` only when the row overview is not enough for a large evaluator report, rejected output, detailed blocker, or review artifact; when a file exists, name it in the receipt row `Overview`.

## Artifact layout

Use this layout for every Doric run:

```text
.doric/
  <MM-YYYY>/
    <DD>/
      <timestamp>-<feature_name>/
        PROMPT.md
        PRD.md
        GAPS_REPORT_PRD.md (when product gaps are found)
        TDD.md
        FEATURES.md
        GAPS_REPORT.md (when technical gaps are found)
        STATE.md
        logs/
          prd_evolution.md
          prd_tournament_evals.json
        agents/ (optional detailed reports)
          001_<phase>_<role>.md
        efforts/
          01_<effort_name>.md
        validation/
          01_<effort_name>.md
        HANDOVER.md
```

When creating a run directory:

- Derive `feature_name` from the user request or agreed feature title.
- Normalize `feature_name` to lowercase snake_case.
- Use the user's current local date and time for `MM-YYYY`, `DD`, and `timestamp`.
- Keep all phase artifacts for that feature inside the run directory, including optional detailed reports, approval evidence, and validation records.

## Workflow

### 1. Prompt and memory bank

Read [references/01-prompt-and-memory-bank.md](references/01-prompt-and-memory-bank.md) for the requirements sub-agent prompt, prompt-to-PRD alignment mode, `PROMPT.md` schema, and completion checks. Write the aligned source of truth to `<run>/PROMPT.md`, then stop before PRD generation to raise and resolve or explicitly defer open questions with the user.

### 2. PRD generation

Start only after `prompt_to_prd_alignment` is approved in `STATE.md`. Read [references/02-prd-generation.md](references/02-prd-generation.md) for the PRD Evolution Engine, multi-PM generation, proximity filtering, Elo tournament, concept grafting, gap analysis, stateless mutation, championship loop, `PRD.md` schema, and guardrails. Write the final zero-gap champion PRD to `<run>/PRD.md`.

### 3. Technical design

Read [references/03-technical-design.md](references/03-technical-design.md) for the architect loop, staged technical evaluators, Dependency Hops, technical tournaments, `GAPS_REPORT.md` handoff, technical design rubric, `TDD.md` schema, and approval gate. Write the final design to `<run>/TDD.md`.

### 4. Decomposition

Read [references/04-decomposition.md](references/04-decomposition.md) for feature extraction, `FEATURES.md`, decomposition validation, graph-based impact analysis, and effort file schema. Write `FEATURES.md` to the run root and effort files to `<run>/efforts/`.

### 5. Development

Read [references/05-development.md](references/05-development.md) for the per-effort red-green-refactor sub-agent roles, circuit breaker, validation rubric, and status update rules. Process one effort at a time.

### 6. Handover

Read [references/06-handover.md](references/06-handover.md) for the handover sub-agent prompt, `HANDOVER.md` schema, and completion checks. Write the final report to `<run>/HANDOVER.md`.

## Commit and rollback policy

- Commit after every green effort with a conventional commit message before starting the next effort.
- Prefer non-destructive recovery: stop, report the failing effort, and preserve evidence.
- Use destructive rollback commands only with explicit user approval and only after verifying the target paths and current status.
