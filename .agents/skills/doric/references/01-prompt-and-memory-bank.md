# Step 1: Prompt and Memory Bank

Use this reference for the Doric requirements elicitation phase. Stop reading once the requirements sub-agent prompt and `PROMPT.md` schema are clear.

## Goal

Create the durable source of truth for the feature run at `<run>/PROMPT.md`, where `<run>` is the active `.doric/<MM-YYYY>/<DD>/<timestamp>-<feature_name>/` directory. After prompt alignment is complete, extract the prompt into product requirements for PRD generation and architecture requirements for technical-design generation. Then stop before PRD generation and enter prompt-to-PRD alignment mode with the user.

Before spawning the requirements sub-agent, the coordinator initializes `<run>/STATE.md` with `Phase: prompt`, `Current effort: none`, `Next effort index: 0`, an `Approvals` table where `prompt_to_prd_alignment`, `tdd_to_decomposition`, and `decomposition_to_implementation` are `no`, and empty `Effort order`, `Active locks`, `Required agents`, `Agent receipts`, `Validation records`, and `Commit checkpoints` tables.

## Requirements sub-agent

Spawn a fresh requirements sub-agent with:

- Role: requirements elicitor
- Ownership: read-only unless the coordinator explicitly asks it to draft `PROMPT.md`
- Inputs: user request, existing user answers, relevant existing Doric artifacts if continuing a run
- Output: structured requirements summary with product/architecture classification, or blocking clarification questions
- Stop condition: enough information exists to write `PROMPT.md`, or the missing facts would materially change the feature definition

## Active listening loop

The requirements sub-agent can run an active-listening loop in Codex by producing concise clarification rounds for the coordinator to ask the user. It does not need conversational filler; it should use structured reflection, targeted questions, and hypothetical edge cases to converge on a durable feature definition.

Each clarification round should include:

- **Reflection:** one or two sentences restating the understood feature, target user, and expected value.
- **Ambiguity check:** the smallest set of missing facts that would materially change the feature definition.
- **Edge-case probe:** one realistic scenario that could invalidate the current framing.
- **Decision capture:** resolved decisions, open questions, non-goals, and product/architecture classification to write into `PROMPT.md`.

Ask concise questions when required. Prefer questions about user value, business driver, personas, constraints, ambiguity, edge cases, and non-goals over implementation details. Sub-agents return questions to the coordinator; the coordinator asks the user, records answers under resolved decisions or open questions, and then resumes the workflow.

## Scoped requirement extraction

Before the prompt phase completes, extract the aligned prompt into two scoped sections:

- **Product requirements:** user value, personas, workflows, business drivers, functional behavior, acceptance expectations, product risks, compliance or user-policy constraints, and non-goals that shape PRD scope.
- **Architecture requirements:** repo constraints, package or boundary constraints, APIs or interfaces, data model or persistence needs, integration points, runtime or UI-state constraints, security and operations constraints, migration or rollout needs, testability constraints, and explicit technical preferences.

If a requirement genuinely affects both downstream phases, duplicate it in both sections with a short note explaining why. Tag open questions as `product`, `architecture`, or `shared` when the tag affects which downstream phase must resolve them.

Register the requirements elicitor in `STATE.md` `Required agents` before spawning it. Add a matching `Agent receipts` row with spawn proof and a short `Overview`, then mark the required-agent row `accepted` before entering prompt-to-PRD alignment mode.

## Prompt-to-PRD alignment mode

After `<run>/PROMPT.md` is drafted and the requirements receipt row is accepted, the coordinator stops the workflow and talks with the user before PRD generation. Do not spawn PRD writer or PRD evaluator agents during this mode.

The coordinator presents a concise alignment checkpoint with:

- The feature summary and user value that will feed the PRD.
- Resolved product, architecture, and shared decisions.
- Every open question from `PROMPT.md`, grouped by `product`, `architecture`, and `shared`, including questions the coordinator thinks are non-blocking.
- A proposed default or deferral only when the current artifacts make that default defensible.
- A direct request for the user to answer, revise, or explicitly accept each remaining open question as non-blocking for PRD generation.

Record each user answer in `PROMPT.md` as a resolved decision or as an explicitly user-accepted non-blocking deferral. Add an `Agent receipts` row such as `alignment_prompt_to_prd` whose `Overview` summarizes the questions raised, the user answers or accepted deferrals, and the coordinator decision. Set `STATE.md` `Approvals` row `prompt_to_prd_alignment` to `yes` only after the user confirms the prompt is aligned for PRD generation.

Do not treat silence, coordinator assumptions, or sub-agent confidence as approval. If the user changes the feature scope or answers a question that materially changes requirements, update `PROMPT.md` and rerun the requirements elicitor when fresh elicitation is needed before asking for prompt-to-PRD alignment again.

## PROMPT.md schema

```markdown
# Prompt

## Feature summary

## User value

## Business or commercial driver

## Personas

## Constraints

## Product requirements

## Architecture requirements

## Active listening notes

## Resolved decisions

## Open questions

## Prompt-to-PRD alignment

## Non-goals
```

## Completion checks

- `PROMPT.md` lives directly inside the Doric run directory.
- `STATE.md` exists and records `Phase: prompt` until the coordinator advances it.
- A requirements agent receipt row exists in `STATE.md` `Agent receipts`, includes spawn proof, is marked `accepted` in both the receipt row and `STATE.md` `Required agents`, and no active prompt required-agent row is pending, spawned, blocked, or rejected.
- The feature summary is specific enough to name the work.
- Product requirements and architecture requirements are separated, and shared requirements are duplicated with a reason instead of left ambiguous.
- Active listening notes record the final reflection, important edge-case probes, and any assumptions accepted by the user.
- Resolved decisions and open questions are separate.
- Open questions are tagged as product, architecture, or shared when the tag affects downstream ownership.
- `PROMPT.md` records the prompt-to-PRD alignment summary, including user answers and accepted deferrals.
- An alignment receipt row exists in `STATE.md` `Agent receipts`, records every open question raised to the user in its `Overview`, and has coordinator decision `accepted`.
- `STATE.md` `Approvals` has `prompt_to_prd_alignment` set to `yes` with the alignment receipt as evidence before the coordinator advances to PRD generation.
- Non-goals constrain later PRD and implementation scope.
- No open question is silently carried into PRD generation. Each open question is resolved, explicitly accepted by the user as a non-blocking deferral, or remains blocking with `Phase: prompt`.
