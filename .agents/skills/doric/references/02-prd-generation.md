# Step 2: PRD Generation

Use this reference for the Doric product requirements phase. Stop reading once the PRD Evolution Engine stages, product gap rules, mutation guardrails, artifact schema, and completion checks are clear.

## Goal

Create `<run>/PRD.md` from the product-scoped parts of `<run>/PROMPT.md` and user answers. The final PRD must be the current zero-gap Champion selected by the PRD Evolution Engine: it preserves the user's value framing, exposes product assumptions as testable User Value Hops, and records why the selected product path is better than serious alternatives.

Use `PROMPT.md` product requirements, user value, business or commercial driver, personas, product-relevant resolved decisions, user-confirmed non-blocking product-tagged or shared open questions, and non-goals as primary inputs. Treat architecture requirements as constraints only when they affect feasibility, compliance, or user-facing scope; do not turn architecture notes into implementation design inside the PRD.

## Preconditions

Do not start PRD generation until `STATE.md` has `prompt_to_prd_alignment` approved with an alignment receipt row in `Agent receipts`. The receipt `Overview` must show that the coordinator raised every `PROMPT.md` open question to the user and recorded each answer, revision, or user-accepted non-blocking deferral. If this evidence is missing, return to prompt-to-PRD alignment mode instead of spawning the PRD phase orchestrator.

## PRD phase orchestrator

The main Doric coordinator does not run product drafting, evaluation, tournament, grafting, gap-analysis, mutation, or championship loops locally. For Step 02, the coordinator only:

- Verifies the preconditions above.
- Registers `PRD phase orchestrator` as the Step 02 required role in `STATE.md`.
- Spawns the phase orchestrator with a narrow prompt and without `fork_context`.
- Receives blocking product questions from the phase orchestrator, asks the user, records the answer or user-accepted deferral, and resumes delegation.
- Reviews the final phase-orchestrator receipt, verifies child-role evidence, marks the required row accepted or rejected, and advances the phase gate.

Spawn the phase orchestrator as a worker with:

- Role: PRD phase orchestrator
- Read ownership: `<run>/STATE.md`, `<run>/PROMPT.md`, accepted prompt alignment receipt row, current Step 02 reference, and existing PRD-phase receipt rows when resuming
- Write ownership: `<run>/PRD.md`, `<run>/GAPS_REPORT_PRD.md`, `<run>/logs/prd_evolution.md`, `<run>/logs/prd_tournament_evals.json`, PRD-phase `Agent receipts` rows, and a PRD child-agent ledger in `STATE.md`
- Inputs: product-scoped `PROMPT.md` sections, user answers, product-relevant constraints, loop limits, and accepted prior Champion evidence when resuming
- Output: final zero-gap `<run>/PRD.md` plus a phase-orchestrator receipt row whose `Overview` names the Champion version, child receipt ids, tournament result, gap-analysis result, mutation attempts, files changed, and recommendation to advance or block
- Stop condition: PRD completion checks pass, a blocking product question needs the user, child spawning fails, guardrails stop the engine, or no acceptable zero-gap Champion can be produced

The phase orchestrator and every child agent use artifact-only context. Do not use `fork_context`; if hidden coordinator context seems necessary, the phase orchestrator stops and asks the main coordinator to record the missing fact in `PROMPT.md`, `PRD.md`, `GAPS_REPORT_PRD.md`, or `STATE.md` before continuing.

The phase orchestrator may spawn and coordinate child agents for generation, proximity filtering, tournament evaluation, concept grafting, gap analysis, stateless mutation, and championship matches. It must register each child role before spawning it in a child required-agent ledger, then record the child agent id, spawn proof, receipt id, reviewed output, and accepted/rejected/blocked decision in `STATE.md`. Coordinator-authored summaries do not satisfy child delegation evidence.

## Engine artifacts

Use these Step 02 artifacts:

```text
<run>/
  PROMPT.md
  PRD.md
  GAPS_REPORT_PRD.md (only when the latest product gap analysis found a blocking gap)
  STATE.md
  logs/
    prd_evolution.md
    prd_tournament_evals.json
```

`logs/prd_evolution.md` stores compact candidate, Champion, Challenger, mutation, and promotion summaries. `logs/prd_tournament_evals.json` stores a JSON-compatible tournament ledger with candidate ids, pairwise outcomes, rule scores, Elo updates, and final rankings. Keep logs concise enough for future agents to consume as artifact context.

## Runtime guardrails

The PRD phase orchestrator enforces these limits:

- Parallel seed count: exactly 3 PM child agents.
- Proximity duplicate threshold: discard candidates whose feature-set structure is more than 85% similar to a higher-scored candidate.
- Initial Elo: 1000 for every unique candidate.
- Elo K-factor: 32.
- Challenger loss circuit breaker: stop after 3 consecutive Challenger losses or duplicate mutations.
- Mutation similarity boundary: a Challenger less than 5% different from the current Champion is a duplicate and counts as a failed mutation attempt.
- Product gap report cleanliness: before each fresh product gap-analysis pass, replace `<run>/GAPS_REPORT_PRD.md` with only the latest blocking gap, or remove the active gap state when the Champion has zero gaps. Preserve historical gap summaries only in `logs/prd_evolution.md`.

If a guardrail stops the engine, keep the last verified Champion, mark the PRD phase orchestrator receipt row `blocked` or `rejected`, and record the smallest next user decision in the receipt `Overview`.

## Step 2.1: Parallel Multi-PM Generation

The phase orchestrator spawns three PM child agents in parallel. Each child receives only artifact paths, recorded decisions, and its persona overlay:

- `PM security/compliance advocate`: prioritizes encryption, zero-trust access, auditability, privacy, data minimization, and local-first data isolation when relevant.
- `PM lean-UX advocate`: prioritizes low-friction workflows, minimal user input, low cognitive load, clear recovery paths, and ordinary integration patterns when relevant.
- `PM scale/performance advocate`: prioritizes lazy loading, delta-state transfer, compressed payloads, asynchronous execution, latency budgets, and operational resilience when relevant.

Every PM draft must follow the `PRD.md` schema, include sequential and testable User Value Hops, and include a self-evaluation score plus the persona-specific tradeoffs it accepted. PM agents must not propose features that violate prompt non-goals, hard constraints, or explicit user decisions.

Record each PM child result in `STATE.md` `Agent receipts`, then summarize the candidate id, persona, self-evaluation score, User Value Hop count, and primary differentiator in `logs/prd_evolution.md`.

## Step 2.2: Proximity Filtering

The phase orchestrator spawns a stateless `proximity filter` child agent after all three PM drafts are available.

The proximity filter compares candidates using structural diff analysis over:

- Functional requirements.
- User Value Hops.
- Personas and primary workflows.
- Acceptance criteria.
- Non-goals and exclusions.
- Risk/compliance treatment.

Do not require external embeddings or vector services. If two drafts are more than 85% structurally similar, discard the duplicate with the lower PM self-evaluation score. If the scores tie, discard the draft with weaker prompt traceability. If only one unique candidate remains, skip the pairwise tournament and treat that candidate as the top candidate for concept grafting.

Record the unique candidate list and discarded candidate reasons in `logs/prd_evolution.md` and the proximity filter receipt `Overview`.

## Step 2.3: Pairwise Elo Tournament

If two or more unique candidates remain, the phase orchestrator spawns a `tournament evaluator` child agent for pairwise comparisons.

The evaluator judges every candidate pair using:

- Security and compliance.
- UX friction.
- Complexity.
- NFR compliance.
- Rule Adherence Score across the five product gap rules.

Rule Adherence Score is the weighted sum of each rule's adherence value in `[0, 1]`. Use these weights:

| Rule | Weight |
| ---- | ------ |
| Rule 1: Direct Coverage | 1.0 |
| Rule 2: Scope Containment | 1.0 |
| Rule 3: Constraint Contradiction | 1.5 |
| Rule 4: NFR Feasibility | 1.0 |
| Rule 5: Implicit Dependency | 0.8 |

The candidate with the higher combined tournament score wins the match. Update Elo with `R_new = R_old + K * (S - E)`, where `K = 32`, `S` is `1` for a win, `0.5` for a tie, and `0` for a loss. If the combined scores tie, choose the candidate with lower product complexity as the match winner and record the tie-breaker.

Write the tournament ledger to `<run>/logs/prd_tournament_evals.json`. Record the ranked array and top two candidate ids in `logs/prd_evolution.md`.

## Step 2.4: Concept Grafting

The phase orchestrator spawns an `evolution PM` child agent.

If at least two unique candidates exist, the Evolution PM takes the two highest-Elo drafts and performs semantic grafting. It combines the strongest compatible concepts from both drafts into one offspring while preserving prompt constraints and non-goals. If only one unique candidate exists, the Evolution PM normalizes that candidate into the initial Champion without inventing fake alternatives.

The offspring becomes `Current Champion v1`. Record:

- Source candidate ids.
- Grafted concepts.
- Concepts rejected as incompatible or out of scope.
- Champion version.
- PRD sections changed.

Store the Champion summary in `logs/prd_evolution.md`; write the Champion draft to `<run>/PRD.md` for the next gate.

## Step 2.5: Deep Verification Gap Analysis

The phase orchestrator spawns a `gap evaluator` child agent to run the zero-blocker gate against the current Champion.

Run the five rules sequentially and stop on the first blocking failure:

1. Direct Coverage: every explicit requirement, business goal, and targeted persona in `PROMPT.md` must trace to a feature, workflow, acceptance criterion, or explicit exclusion in `PRD.md`.
2. Scope Containment: no PRD feature may intersect with prompt non-goals, out-of-scope items, or user-deferred product behavior.
3. Constraint Contradiction: no PRD requirement may contradict prompt constraints, compliance constraints, platform limits, or recorded user decisions.
4. NFR Feasibility: User Value Hops must be practically capable of meeting prompt NFRs such as latency, storage, security, scale, or availability under stated target conditions.
5. Implicit Dependency: the PRD must not depend on a system capability unless the capability exists in the current codebase, is supplied by an accepted constraint, or is specified as buildable product scope in the PRD.

If the Champion has zero blocking gaps, the phase orchestrator records a zero-gap Champion result and may complete Step 02. If a blocking gap exists, write `<run>/GAPS_REPORT_PRD.md` using this schema:

```markdown
# PRD Gaps Report

## [GAP_ID: PRD_ERR_<rule-number>_<short-code>]

- **Rule Violated:** Rule <number> (<name>)
- **Source Prompt Node:** `<PROMPT.md section or heading>`
- **Target PRD Node:** `<PRD.md section or heading>`
- **Failure Description:** <specific failure>
- **Resolution Requirement:** <minimal product mutation needed>
```

Record the gap evaluator decision in `STATE.md` and summarize the gap in `logs/prd_evolution.md`.

## Step 2.6: Stateless Mutation

If `GAPS_REPORT_PRD.md` contains a blocking gap, the phase orchestrator spawns a fresh `mutation PM` child agent. The mutation PM receives only:

- `<run>/PROMPT.md`
- Current `<run>/PRD.md` Champion
- Current `<run>/GAPS_REPORT_PRD.md`
- Relevant receipt ids and recorded user decisions

The mutation PM must produce a Challenger draft that targets the exact gap report. It changes only the PRD sections required to satisfy the `Resolution Requirement` and preserves already valid Champion behavior. It must not defend the previous Champion from memory, reuse hidden context, or widen product scope to make the fix easier.

If the Challenger is less than 5% structurally different from the Champion, mark it as a duplicate mutation, count it as a failed challenger attempt, and spawn a new mutation PM unless the circuit breaker has fired.

Record the Challenger version, target gap id, changed PRD sections, and duplicate status in `logs/prd_evolution.md`.

## Step 2.7: Championship Match

The phase orchestrator spawns a fresh `championship evaluator` child agent to compare Challenger vs Champion.

The evaluator decides whether:

- The Challenger resolves the gap in `GAPS_REPORT_PRD.md`.
- The Challenger preserves or improves the Champion's core UX, security, performance, and compliance values.
- The Challenger avoids new NFR violations, scope leaks, contradictions, or implicit dependencies.

If the Challenger wins, promote it to `Current Champion v<N+1>`, write it to `<run>/PRD.md`, reset consecutive challenger losses to `0`, and recurse to Step 2.5 for a fresh zero-blocker gap analysis.

If the Challenger loses, discard it, increment consecutive challenger losses, record the reason in `STATE.md` and `logs/prd_evolution.md`, and return to Step 2.6 with a fresh mutation PM unless the circuit breaker has fired.

## PRD rubric

The final Champion must satisfy:

- Requirements are concrete, testable, and unambiguous.
- Personas and primary workflows are explicit.
- Generator persona tradeoffs are represented in final decisions.
- Acceptance criteria cover success, failure, empty, and edge states.
- User Value Hops are sequential, testable, and independently evaluated.
- Prompt requirements, business goals, and target personas have direct coverage.
- Non-goals and out-of-scope boundaries prevent speculative implementation.
- Prompt constraints and compliance requirements are not contradicted.
- NFRs are feasible against the User Value Hops.
- Implicit dependencies are either provided, specified as buildable scope, or removed.
- KPIs or success measures are measurable.
- Tournament and evolution results are present when credible alternatives existed.
- Open questions are separated from resolved decisions.

## PRD.md schema

```markdown
# Product Requirements Document

## Problem statement

## Goals

## Non-goals

## Personas

## Generator persona debate

## Primary workflows

## Functional requirements

## User Value Hops

## Acceptance criteria

## Product alternatives and tournament

## Evolution summary

## Success measures

## Risks and compliance

## Open questions
```

## User-question stop condition

If any child agent identifies a blocking product question, the phase orchestrator stops the engine and returns a blocked phase receipt row to the main coordinator. The receipt `Overview` must include the exact question, why it blocks the PRD, the affected PRD sections or User Value Hops, and the minimal context needed for the coordinator to ask the user. No child agent may treat an unanswered blocking question as resolved. After the coordinator records the user answer or accepted non-blocking deferral, spawn or resume the phase orchestrator with that evidence.

## Final handoff

The phase orchestrator's final `Agent receipts` row must include:

- Spawn proof for the `PRD phase orchestrator` role and the matching `STATE.md` required-agent row.
- Champion version and zero-gap result.
- Child required-agent ledger with role, agent type, agent id, receipt id, status, and coordinator/orchestrator decision for every PM, filter, evaluator, mutation, and championship child.
- Tournament summary, top candidates, graft summary, and final Champion rationale.
- Mutation attempt count, consecutive challenger losses, and circuit breaker status.
- Active or cleared `GAPS_REPORT_PRD.md` status.
- A compact `Overview` covering product-scoped input summary, files changed, remaining non-blocking open questions, and recommendation to advance to technical design or reject the PRD.

## Completion checks

- `PRD.md` lives directly inside the Doric run directory and is the latest zero-gap Champion.
- `logs/prd_evolution.md` records candidate generation, filtering, tournament, grafting, gaps, mutations, championship outcomes, and final Champion rationale.
- `logs/prd_tournament_evals.json` records the tournament ledger when two or more unique candidates existed.
- `GAPS_REPORT_PRD.md` is absent, empty of active blocking gaps, or explicitly marked superseded after the final zero-gap Champion.
- The main coordinator has accepted the `PRD phase orchestrator` required-agent row and receipt for the completed PRD phase.
- Accepted child evidence exists for generation, proximity filtering, tournament when applicable, concept grafting, gap analysis, mutation when applicable, and championship when applicable.
- Child evidence includes spawn proof, agent id, receipt id, role decision, and `Overview` text strong enough to prove delegation was not replaced by coordinator summary.
- No active PRD phase-orchestrator row or child role is pending, spawned, blocked, or rejected.
- `STATE.md` records `Phase: prd` while PRD generation is active and advances only after the zero-gap Champion handoff is accepted.
- Every goal traces back to product-scoped requirements in `PROMPT.md`.
- Architecture requirements are used only as feasibility, compliance, or user-facing scope constraints.
- Remaining open questions do not block technical design.
