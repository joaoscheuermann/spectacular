# P0 Planning Ablation

This paid oracle final-synthesis experiment compares direct plan synthesis with
P0-aware revision as assessed by the primary judge. The objective,
author-labeled oracle relevant-skill bundle, model, reasoning effort, and output
contract are held constant, while each arm receives a purpose-specific system
prompt. It does not test skill retrieval or gating.

The two arms are:

- `withoutP0`: objective plus the bodies of the gold skills;
- `withP0`: the byte-identical content above followed by a `P0 Draft` block.

The planning call in `withoutP0` is genuinely independent of P0: P0 is neither
generated nor used to choose its skills, construct its prompt, or produce its
plan, and its system prompt never mentions P0. Both planning calls always
execute in fresh contexts, using separate user-prompt builders and
purpose-specific system prompts.

## Hypothesis

For cases with a stable preference for exactly one arm:

```text
H0: P(primary judge prefers withP0) <= 0.5
H1: P(primary judge prefers withP0) > 0.5
alpha = 0.05
```

The decision uses an exact one-sided binomial test. The two A/B orientations of
one case are consistency checks, not independent observations. If there are no
stable single-arm preferences, the decision is
`insufficient_stable_preferences`; otherwise it is `reject_null` or
`fail_to_reject_null`.

## Frozen inputs and oracle bundle

P0 comes only from the versioned [`fixtures/p0.json`](fixtures/p0.json). It was
projected from `name`, `objective`, and `p0` in this source result:

```text
run ID: 89ba6c1e-470c-43ea-809b-a34a90f59540
results.json SHA-256: f904b241cc05264c281e017ebfb16cbe52af934c4d4c7bc6b62f71088f5b68db
frozen fixture SHA-256: 909bfc7d5153a08bfbe516604d68b3255905bbddb267e4334aed7ec94a7d4908
```

Normal generation execution never reads the source run or regenerates P0.
Before provider construction, it verifies that the fixture contains exactly
one nonempty P0 for each of the 30 local cases, has no unknown cases, and
matches every local name and objective. The fixture hash and the complete
37-skill case classification are fail-closed as well.

For each case, the oracle bundle is the deterministic ordered union:

```text
skills.expected + skills.useful
```

Every name must exist in the 37-skill local catalog and the union must contain
no duplicates. The same frozen array is used to render byte-identical skill
bodies and order for both planners and both judge orientations. The `expected`
and `useful` category labels build this author-labeled oracle treatment but are
not included in any model prompt. The union is a controlled relevant-skill
bundle, not a claim that it is minimal or globally optimal. `skills.noise`
participates only in fail-closed corpus validation; it has no treatment or
metric role after that validation.

## Planning contract

Both arms use the same planning model, reasoning effort, schema, flags, and
output contract. The `withoutP0` system prompt requests direct synthesis and
contains no P0 or revision language. The `withP0` system prompt requests review
of the supplied fallible draft and permits its goals to be removed, corrected,
split, merged, reordered, or completely reconstructed. Separate
`p1WithoutP0User` and `p1WithP0User` builders make the treatments explicit.
Their user messages share this byte-identical prefix:

```markdown
# Objective

...

# Selected Skills

...
```

Only `withP0` appends:

```markdown
# P0 Draft

...
```

In both prompts, the objective is the sole authority for scope and deliverables.
Skill bodies are advisory operational guidance: materially applicable methods
and checks may improve the plan, but cannot introduce unsupported scope. In the
revision prompt, P0 is explicitly fallible and may be removed, corrected,
split, merged, reordered, or entirely reconstructed. Every result must be a
complete standalone plan of observable, verifiable outcomes, without
skill-name or tool-invocation goals.

## Blind primary judgment

Only the configured primary judge is used in this economic first cut. It makes
two calls per case. The first orientation randomly assigns the plans to Option
A and Option B; the second swaps those options exactly.

The judge receives only:

- the objective;
- the same ordered gold skill bodies;
- Option A; and
- Option B.

It never receives P0, arm identities, gold category names, prior-run results,
traces, or experiment metadata. Its rubric prioritizes, in order, objective
fidelity without invented scope, complete objective coverage, correct use of
only materially applicable skill guidance, and observable verification. Length
and detail count are not quality signals.

The two orientations must map to the same semantic outcome:

- `withoutP0`;
- `withP0`;
- `both`; or
- `neither`.

Otherwise the case is `inconsistent`. Byte-identical plan arrays are classified
as `both` after both judge calls have still been made; the raw judge outcome is
also retained for auditability.

## Metrics and call budget

The run reports all outcome counts, decisive stable preferences,
`withP0PreferenceRate`, the exact one-sided binomial p-value, and the hypothesis
decision. It also reports local-case, catalog, frozen-P0, P0-goal, gold-bundle,
gold-skill occurrence, and unique-gold-skill counts.

Provider usage includes tokens and provider-reported costs both in aggregate
and separately for the operations below. It also records attempted calls by
operation, including retries; token and cost data cover only completed attempts
whose provider result exposes usage.

- `p1_without_p0`;
- `p1_with_p0`;
- `judge`.

A successful 30-case run therefore records exactly 120 successful calls:

```text
30 p1_without_p0 + 30 p1_with_p0 + 60 judge
```

There are no indices, embeddings, lexical or vector search, hybrid fusion,
reranking, gate calls, retrieval configuration, bundle scoring, or associated
traces in this experiment.

## Cross-judge rejudgment

The separate rejudge entry point completes the cross-judge cells without
regenerating either plan. Its versioned
[`fixtures/rejudge.json`](fixtures/rejudge.json) pins two completed generation
runs, their complete manifest and result hashes, the source and target judges,
and a comparison-contract hash covering the byte-level prompt, structured
choice contract, and provider flags:

| Frozen plan batch                      | Source judge              | Rejudge target            |
| -------------------------------------- | ------------------------- | ------------------------- |
| `cfab07af-5551-4e5c-85b4-d418ccc302c5` | `google/gemini-3.7-flash` | `openai/gpt-5.6-sol`      |
| `3541de47-9a14-4f54-911a-06a0d9eb3de7` | `openai/gpt-5.6-sol`      | `google/gemini-3.7-flash` |

Before constructing a provider, rejudge verifies those hashes, completed-run
schemas, run IDs, source-judge identity, frozen fixture, local case and catalog
hashes, every objective, P0, gold-skill name and order, persisted plan and
outcome consistency, and the current comparison protocol. It reuses each
source case's exact first and inverted A/B positions.

The source runs predate this standalone campaign fixture and came from a dirty
worktree; they retain an aggregate source hash but not a separately persisted
comparison-contract hash. The fixture therefore records a reviewed historical
attestation from each pinned source hash to the comparison contract. Runtime
validates that mapping and the current contract fail-closed, but cannot derive
the historical mapping from the ignored source artifacts alone. Under this
explicit provenance limitation, the target judge receives the same objective,
ordered skill bodies, options, rubric, schema, effort, and flags as the source
judge; only the judge model changes. Generation outputs created by the current
runtime persist the contract hash directly.

Each cell makes 60 successful `rejudge` calls and zero planning calls. Its
standalone output records `mode: rejudge`, complete source lineage, a SHA-256
of each frozen plan pair, source and target outcomes, the outcome transition
matrix, and exact, stable, and decisive-direction agreement. The two plan
batches are repeated generations over the same 30 cases, so they must be
reported as two robustness cells rather than pooled as 60 independent cases.

## Persistence and run

Set `OPENROUTER_API_KEY` and run:

```sh
npm run llm:p0-planning-ablation
```

Run either pinned cross-judge cell with:

```sh
npm run llm:p0-planning-rejudge -- \
  --source-run cfab07af-5551-4e5c-85b4-d418ccc302c5 \
  --judge-model openai/gpt-5.6-sol

npm run llm:p0-planning-rejudge -- \
  --source-run 3541de47-9a14-4f54-911a-06a0d9eb3de7 \
  --judge-model google/gemini-3.7-flash
```

The pinned source directories must be present under the ignored local
`output/` tree. Rejudge never changes them, and a fresh checkout cannot run
this campaign from versioned files alone.

Each invocation that passes input validation creates an ignored
`output/<run-id>/` directory with:

- `manifest.json`: UUID, status, effective configuration, Git identity,
  environment, and separate SHA-256 identities for the P0 fixture, cases,
  catalog, experiment sources, and package metadata;
- `output.log`: the Pino JSON Lines trace, while the same events are rendered
  separately in a readable console format;
- `results.json`: objective, frozen P0, gold skill names, both plans, two
  oriented judgments, stable outcome, corpus and comparison metrics, and usage.

Provider actions use up to five total attempts with waits of 15, 30, 60, and
120 seconds before attempts two through five. Credentials are never persisted.

## Interpretation boundary

This experiment compares direct synthesis with P0-aware revision during oracle
final synthesis. It does not isolate the semantic effect of P0 from the
revision-specific system framing, added context length, or structural scaffold,
and it does not establish downstream task-execution success. The result can
depend on P0 quality. The authored cases are not a random population.
There is only one generated plan per arm and case in each batch; the repeated
orientations measure judge consistency over those same plans, not generation
stability. Cross-judge rejudgment measures evaluator robustness but still does
not establish downstream quality. End-to-end execution confirmation remains a
separate follow-up stage.

## Related work

These papers motivate the ablation but do not answer its exact same-model,
gold-skill-conditioned comparison:

- **[Revision or Re-Solving? Decomposing Second-Pass Gains in Multi-LLM Pipelines](https://arxiv.org/abs/2604.01029)**
  (COLM 2026) separates second-pass gains into re-solving, structural
  scaffold, and draft content. It is the closest methodological precedent for
  holding final-call evidence constant while ablating draft exposure.
- **[Self-Refine: Iterative Refinement with Self-Feedback](https://proceedings.neurips.cc/paper_files/paper/2023/hash/91edff07232fb1b55a505a9e9f6c0ff3-Abstract-Conference.html)**
  (NeurIPS 2023) conditions iterative generation on an initial output and
  feedback. It supports draft-conditioned refinement, but does not isolate the
  draft while holding external guidance constant.
- **[Describe, Explain, Plan and Select: Interactive Planning with LLMs Enables Open-World Multi-Task Agents (DEPS)](https://proceedings.neurips.cc/paper_files/paper/2023/hash/6b8dfb8c0c12e6fafc6c256cb08a5ca7-Abstract-Conference.html)**
  (NeurIPS 2023) revises plans using environment feedback. It provides
  plan-specific evidence for feedback-guided revision, although its feedback
  arises after interaction rather than from fixed pre-execution skills.
- **[Large Language Models Cannot Self-Correct Reasoning Yet](https://proceedings.iclr.cc/paper_files/paper/2024/hash/8b4add8b0aa8749d80a34ca5d941c355-Abstract-Conference.html)**
  (ICLR 2024) finds that intrinsic self-correction without external feedback
  can fail or degrade reasoning. It supports treating P0 as fallible rather
  than authoritative.
- **[SCREWS: A Modular Framework for Reasoning with Revisions](https://arxiv.org/abs/2309.13075)**
  (2023 preprint) shows that revision can introduce errors and that selection
  between candidates matters, motivating blind regression-sensitive judgment.
- **[Chain-of-Verification Reduces Hallucination in Large Language Models](https://aclanthology.org/2024.findings-acl.212/)**
  (Findings of ACL 2024) separates verification from initial generation,
  motivating separation of independent verification from draft-conditioned
  synthesis.

The earlier
[Direct Skill Planning](../direct-skill-planning/README.md#factor-comparisons)
diagnostic provides internal exploratory evidence, but used only a six-case
subset of this corpus with a different prompt, model, and retrieval pipeline.
This experiment removes that retrieval path and compares direct synthesis with
P0-aware revision under the author-labeled oracle bundle.
