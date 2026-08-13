# Handoff: skill-selection and composition benchmark

## Status

This document is an implementation proposal, not an active benchmark contract.
It does not authorize a paid run, change the current SkillsBench decision gate,
or replace the pinned canonical campaign described in `GROUNDING.md`.

The next agent must read `GROUNDING.md`, the root `AGENTS.md`, this benchmark's
current source and tests, and the upstream benchmark artifacts before changing
code. Any accepted implementation changes benchmark runtime behavior and must
update `GROUNDING.md`.

## Problem to answer

The canonical SkillsBench campaign gives each task its curated task skills. It
is a valid and deliberately conservative comparison, but it reduces the
selection problem: a direct agent may receive only a small, already relevant
skill set.

The additional experiment should answer a different, narrower question:

> With the same model, tasks, tools, and candidate skill library, does MOSAIC
> select and compose skills well enough to achieve a higher public-benchmark
> reward at a lower total model cost than the equivalent direct agent?

Fair treatment and construct relevance are separate. The existing canonical
campaign establishes the former. This experiment should increase the latter
without choosing tasks or skills after observing MOSAIC's results.

## Recommended decision

Keep the current evaluation ladder unchanged and add one separate,
SkillsBench-only composition condition:

1. Canonical SkillsBench remains the primary public result.
2. The existing one-task smoke and fixed ten-task pilot remain operational and
   diagnostic checks for the canonical condition.
3. A new composition condition exposes the same fixed candidate skill catalog
   to both `mosaic-direct` and `mosaic`.
4. Terminal-Bench remains a secondary transfer test and may still be unlocked
   only by a valid full 87-task canonical SkillsBench report.
5. A composition report must never satisfy that Terminal-Bench gate.

Do not call the new condition canonical SkillsBench. Suggested result and CLI
terminology is `skillsbench composition`, provided that this name remains
unambiguous in metadata and result paths.

## Upstream protocol to reproduce

The closest published protocol is **Generative Skill Composition for LLM
Agents (SkillComposer)**. It frames skill use as a joint choice of subset,
cardinality, and order over a fixed library. Its project page reports:

- 9,872 task-composition records;
- a 196-skill human-curated library;
- downstream execution on 75 SkillsBench tasks;
- `no skills`, `all skills`, retrieval top-3, predicted composition, and
  gold-skill conditions;
- task pass rate and prompt-token comparisons.

Primary sources:

- Project: <https://skill-composer.github.io/>
- Paper: <https://arxiv.org/abs/2606.32025>

Important blocker observed on 2026-08-10: the project's visible `Code` link
resolved back to the project page, not to a public code repository. The future
agent must locate and verify an official repository, immutable revision,
license, 196-skill catalog, task mapping, and evaluation split before claiming
to reproduce SkillComposer. Do not reconstruct missing upstream artifacts from
the paper and present them as the published benchmark.

The paper's reported 75-task downstream split also differs from this
repository's pinned 87-task SkillsBench v1.1 contract. Resolve the exact task
IDs and upstream versions before implementation. Do not silently approximate
the split.

## Catalog choices

Choose exactly one catalog source before any paid result is inspected.

### Preferred: official SkillComposer catalog

Use this only if the official artifacts become publicly available and their
license permits redistribution or pinned download. Record:

- repository URL and immutable commit;
- catalog path and catalog SHA-256;
- every skill ID, source path, and content digest;
- exact evaluation task IDs and their provenance;
- any declared dependency or ordering labels.

### Fallback: pinned SkillsBench-wide catalog

If the official SkillComposer artifacts remain unavailable, a defensible but
distinct experiment can build a global catalog from all valid `SKILL.md`
packages in the already pinned SkillsBench commit
`b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af`.

This fallback must be labeled a repository-defined composition condition, not
the SkillComposer benchmark. Generate the catalog deterministically before any
model run, preserve supporting files, handle duplicate names with a stable
namespacing rule, and persist a manifest containing the source path and digest
of every package. Both arms must receive the byte-identical catalog, without a
gold task-to-skill mapping.

Do not combine the official and fallback catalogs, add hand-picked distractors,
or change the catalog after observing outcomes.

## Experimental contract

### Arms

Retain the two current arms:

| Arm             | Treatment                                                                             |
| --------------- | ------------------------------------------------------------------------------------- |
| `mosaic-direct` | Existing minimal direct model-and-terminal loop with access to the full fixed catalog |
| `mosaic`        | Existing public MOSAIC entrypoint with access to the same full fixed catalog          |

Do not use `mosaic/evaluation` hooks, hidden task labels, gold skill mappings,
or a different retrieval index in only one arm. MOSAIC's normal planning and
routing are the treatment.

### Fixed neutral controls

Preserve the current closed campaign controls unless a separately approved
contract changes them:

- provider route: OpenRouter through BenchFlow's LiteLLM proxy;
- model: `openrouter/deepseek/deepseek-v4-pro-0813`;
- adapter-owned reasoning effort: `low` for both arms;
- terminal tool and Docker sandbox;
- single-shot loop, zero retries, concurrency 1, build concurrency 1;
- required trusted usage and positive cost telemetry;
- identical task prompts, task files, candidate catalog, and public verifiers;
- sequential arms from one campaign ID.

### Decision metric

Keep the existing strict Pareto decision and introduce no composite score:

```text
mean_reward(mosaic) > mean_reward(mosaic-direct)
AND
total_model_cost(mosaic) < total_model_cost(mosaic-direct)
```

Selection accuracy, selected-skill count, order agreement, tool calls, and
context size may be recorded as diagnostics. They must not replace reward and
cost as the decision gate.

### Task selection

Predeclare the exact task list before any paid composition run. Recommended
stages are:

1. one operational task;
2. a fixed ten-task composition pilot selected without result knowledge;
3. the complete, version-reconciled composition split.

Do not reuse the existing canonical pilot label for a composition pilot. A
canonical pilot and a composition pilot answer different questions and need
different metadata.

## Fairness concern: a deliberately weak direct baseline

Flooding every full skill body into the direct prompt while letting MOSAIC
retrieve lazily may be the intended system comparison, but it can also create
an artificially weak baseline through context overflow. Resolve this before
running, not after seeing results.

The minimal first comparison should remain the two product conditions requested
by the MOSAIC evaluation: direct access to the complete library versus normal
MOSAIC orchestration over the same library. Before claiming that MOSAIC beats
skill retrieval generally, add a separately labeled cheap retrieval baseline,
such as frozen embedding top-k, using the same catalog and no result-informed
tuning. That third baseline is diagnostic and must not be mixed into the
Direct-versus-MOSAIC Pareto gate.

Define and test what happens if the direct prompt exceeds provider context.
Context overflow, truncation, or omitted skills must fail closed rather than be
counted as a valid Direct loss.

## Projected implementation surface

Inspect the current files rather than treating this list as authoritative. The
initial projection is:

```text
benchmarks/mosaic/
  src/
    campaign.ts          add the paid SkillsBench-only composition action
    cli.ts               parse and document the new action
    compare.ts           validate the closed composition evidence
    composition.ts       fixed task/catalog contract, only if shared by multiple consumers
  tests/
    campaign.test.ts     command assembly, payment gate, catalog provenance
    cli.test.ts          dispatch and invalid benchmark/action combinations
    compare.test.ts      valid pair plus fail-closed mutation cases
  agents/
    mosaic-direct/manifest.toml
    mosaic/manifest.toml update only if catalog mounting changes the launch contract
  README.md              commands, scope, cost warning, and interpretation
  GROUNDING.md           update the root contract if implementation is accepted
```

Avoid standalone hand-authored `.mjs` scripts. Continue using the Nx CLI
surface. The only `.mjs` artifact should remain the generated container bundle.

## Required persisted evidence

In addition to the existing metadata, task manifest, run config, health,
bundle, and agent-manifest digests, a composition campaign must prove:

- condition identity distinct from canonical SkillsBench;
- immutable catalog source and revision;
- exact catalog manifest digest;
- exact skill package IDs and content digests;
- exact predeclared task list;
- byte-identical catalog availability in both arms;
- catalog mount or `skills_dir` configuration recorded by the harness;
- no missing, duplicate, unreadable, or malformed selected packages;
- no prompt truncation or context-overflow invalidation in the direct arm.

The comparator must recompute these digests and reject missing or unexpected
fields. Pair validation must reject arms from different campaign IDs,
conditions, task sets, catalogs, bundles, models, or run configurations.

If BenchFlow 0.6.5 cannot express and record the catalog contract, stop and
document the missing capability. Do not silently upgrade BenchFlow or weaken
the comparator. An upgrade changes the pinned harness contract and requires
separate validation and grounding.

## Minimum tests and acceptance criteria

The implementation is not complete until all of the following are proven:

- free preflight performs no model call;
- paid composition actions require `--yes-paid-run` before commands or result
  directories;
- Terminal-Bench rejects the composition action;
- an exact predeclared task set is passed to both arms;
- the catalog revision and digest are pinned and copied into evidence;
- a missing or mismatched catalog fails before paid execution where possible;
- catalog differences between arms make comparison invalid;
- altered task selection, skill digests, run config, health, usage, cost, or
  model identity make comparison invalid;
- composition reports cannot unlock Terminal-Bench;
- failed first arms stop the campaign before the second paid arm;
- current canonical smoke, pilot, run, and comparison behavior remains intact;
- Nx typecheck, tests, build, formatter checks, `nx sync:check`, and
  `git diff --check` pass;
- the generated bundle, both manifest pins, checksum sidecar, and published
  release asset match before a paid run.

Run only a free preflight during implementation. A paid smoke, pilot, or full
composition campaign requires fresh explicit approval.

## Interpretation contract

Do not use the composition condition to rescue a negative canonical result by
changing the claim after the fact. Report both conditions separately:

| Canonical SkillsBench | Composition condition | Supported conclusion                                                   |
| --------------------- | --------------------- | ---------------------------------------------------------------------- |
| MOSAIC wins           | MOSAIC wins           | Evidence for benefit with curated and large candidate sets             |
| MOSAIC loses          | MOSAIC wins           | Benefit is scoped to selection/composition under a large library       |
| MOSAIC wins           | MOSAIC loses          | Current routing does not scale to the tested catalog                   |
| MOSAIC loses          | MOSAIC loses          | Current implementation or hypothesis is unsupported by both conditions |

If the catalog or task split cannot be reproduced, the composition result is
invalid rather than negative.

## Secondary benchmark, not the next implementation

After composition evidence exists, **ComplexMCP** is the strongest current
candidate for testing broader MOSAIC orchestration. It reports more than 300
tools across seven stateful sandboxes, interdependent workflows, deterministic
seeded states, and injected API failures:

- Repository: <https://github.com/ATH-MaaS/complex-mcp>
- Paper: <https://arxiv.org/abs/2605.10787>

It evaluates MCP tools rather than `SKILL.md` packages, so it answers a broader
tool-orchestration question and requires a different adapter. Do not add it to
the same implementation as the SkillsBench composition condition.

Other public candidates are less aligned with the current mechanism:

- SWE-Skills-Bench uses real repositories and deterministic tests, but mainly
  tests the utility of a task-paired skill:
  <https://github.com/GeniusHTX/SWE-Skills-Bench>
- SkillFlow-Bench has 166 runnable Harbor tasks across 20 families, but focuses
  on lifelong skill discovery and evolution:
  <https://huggingface.co/datasets/KouShi2/skillflow-bench>
- SkillLearnBench has 100 verified instances across 20 task types, but focuses
  on generating and continually improving skills:
  <https://github.com/cxcscmu/SkillLearnBench>

## Explicit non-goals

- Do not replace the canonical SkillsBench gate.
- Do not invent a custom aggregate metric.
- Do not choose tasks, distractors, top-k, or catalog size from observed wins.
- Do not claim a repository-defined fallback is the SkillComposer benchmark.
- Do not introduce skill generation, online learning, or cross-run memory.
- Do not combine ComplexMCP with this change.
- Do not execute or publish a paid campaign without explicit approval.
