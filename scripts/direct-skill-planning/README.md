# Direct Skill Planning

This diagnostic lab compares four complete planning policies over the same
user request, skill catalog, planning model, vector index, and reranker:

- **Direct:** retrieve skills once from the original request, then generate a
  final goal list directly from the request and retrieved skill bodies.
- **Direct Goal:** generate a catalog-independent P0, retrieve skills
  independently for every P0 goal, then generate a final goal list directly
  from the original request and the unique union of those skill bodies. The
  final planning call does not receive P0.
- **Request P1:** generate a catalog-independent P0, then revise it with the
  same request-level skills used by Direct.
- **Goal P1:** generate the same P0, retrieve skills independently for every P0
  goal, then revise it with the same unique union used by Direct Goal.

All final planning calls receive the same definition of an observable goal and
the same skill-use rules. They must apply only materially relevant guidance,
ignore irrelevant or conflicting content, express guidance as outcomes or
verification criteria, avoid skill names and skill-invocation goals, preserve
the request, and avoid unsupported assumptions. P0 and all four final planning
policies use explicit `high` reasoning effort. Both judges use `medium`
reasoning effort.

Planning and judging omit `temperature` because reasoning-model endpoints may
not support that control, and strict OpenRouter parameter routing rejects a
request when any transmitted parameter is unsupported.

All policies use `victor` for in-memory cosine vector search with
`voyageai/voyage-4-large` embeddings. Each query produces a vector shortlist of
up to 20 skills. `voyageai/rerank-2.5` returns at most ten, after which the
default `0.30` minimum relevance score may reduce the final set further. The
skill catalog is embedded once per run and reused by every request and goal
query.

The catalog contains 37 skills: eight focused local skills and 29 official
SkillsBench skills. The SkillsBench subset contains the 28 unique skills from
the repository's predeclared ten-task pilot plus `citation-management` for the
citation case. Their complete `SKILL.md` bodies are loaded from SkillsBench
v1.1 commit `b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af`; the duplicate `pptx`
package shared by two pilot tasks is included once. This experiment does not
yet use the complete SkillsBench catalog.

## Hypotheses

The primary comparison remains:

- **Direct × Goal P1:** whether request-level retrieval followed by direct
  generation is preferred to P0, per-goal retrieval, and P1 revision.

Four diagnostic comparisons form the edges of a two-by-two design and separate
the factors mixed by that end-to-end comparison:

- **Direct × Request P1:** both arms receive the same request-level retrieved
  skills, isolating direct generation from P0 anchoring and revision.
- **Direct × Direct Goal:** both arms generate directly, isolating request-level
  retrieval from per-goal retrieval.
- **Request P1 × Goal P1:** both arms revise the same P0 with the same revision
  prompt, isolating request-level retrieval from per-goal retrieval.
- **Direct Goal × Goal P1:** both arms receive the same per-goal retrieved
  skills, isolating direct generation from P0 anchoring and revision.

Request P1 and Direct Goal are not compared because they change both factors in
opposite directions and therefore do not isolate either cause.

Every judge evaluates each pair twice with A/B positions reversed. For one
judge, a pair has a stable outcome only when both orientations map to the same
policy, `both`, or `neither`; otherwise it is `inconsistent`. For each judge
and pair, among stable comparisons where that judge selects exactly one policy:

```text
H0: P(judge prefers the left policy) <= 0.5
H1: P(judge prefers the left policy) > 0.5
```

The first orientation is randomized and the second uses the exact same plans
in reversed positions. Each model judge receives the same plans and
orientations. A judge sees only the initial request, the case's predeclared
expected skills, and the two final goal lists. It does not receive P0,
retrieval traces, policy identities, or the other judge's result. The judges
are `google/gemini-3.7-flash`, matching `scripts/full-skill-vs-hints`, and
`openai/gpt-5.6-sol`.

`both`, `neither`, and position-sensitive `inconsistent` outcomes are reported
separately rather than forced into policy wins. Repeated rounds measure judge
and generation stability but do not turn repeated evaluations of the same
authored cases into independent benchmark tasks.

Every provider operation has up to five total attempts. This covers planning
and judge completions, catalog and query embeddings, and reranking. The unified
provider's own structured-output correction happens inside an attempt; if that
still fails, the lab retries the complete operation. Local programming errors
are not retried. Provider failures explicitly marked non-retryable are recorded
once and exhausted immediately without a retry delay.

The retry schedule is progressive: 5, 10, 20, and 30 seconds before attempts
two through five. The failed-attempt event is persisted before each delay
begins. Independent provider operations remain concurrent; there is no request
pool or leasing layer.

Every failed provider attempt is emitted to the terminal as one safe JSON
record and atomically appended to the run's `providerFailures`. Records contain
a sequence, timestamp, operation, model, provider, safe error code, attempt,
outcome, the next delay in milliseconds when another attempt remains, optional
HTTP status and retryability, plus applicable allowlisted context such as case,
round, arm, retrieval kind, goal index, skill, pair, and orientation. They never
contain HTTP bodies or headers, URLs, prompts, model inputs or outputs,
diagnostics, messages, causes, stacks, or credentials.

## Recorded Results

The checked-in evidence contains two complete judge-mode runs made on
2026-08-28 after correcting the judge prompt:

- [`88a74c54-fa6e-456b-b6ae-b963205667fd.json`](evidence/88a74c54-fa6e-456b-b6ae-b963205667fd.json), SHA-256
  `89508e72020cdfd13acb34235fda53369972bbeff2493bd93c3c54eefd4a676a`;
- [`aaaa645e-a9b2-4d2c-a3bd-119f13cc7879.json`](evidence/aaaa645e-a9b2-4d2c-a3bd-119f13cc7879.json), SHA-256
  `23b235cc0cfe3a79a264f28d7fdf82a66b8816f1647cafff7419a42bf2dfae0e`.

Both runs completed all 18 case-round evaluations with no provider failures.
They used the same six authored cases, three rounds, planning model
`deepseek/deepseek-v4-pro`, judges `google/gemini-3.7-flash` and
`openai/gpt-5.6-sol`, top-10 reranking, and the `0.30` minimum reranker score.

### Primary Comparison

The primary Direct versus Goal P1 outcomes were:

| Run           | Judge            | Direct | Goal P1 | Inconsistent |
| ------------- | ---------------- | -----: | ------: | -----------: |
| `88a74c54…`   | Gemini 3.7 Flash |      3 |      12 |            3 |
| `88a74c54…`   | GPT-5.6 Sol      |      5 |      12 |            1 |
| `aaaa645e…`   | Gemini 3.7 Flash |      5 |      11 |            2 |
| `aaaa645e…`   | GPT-5.6 Sol      |      7 |       9 |            2 |
| **Aggregate** | **Both judges**  | **20** |  **44** |        **8** |

Goal P1 therefore received 44 of the 64 stable single-policy selections
(68.75%). This diagnostic evidence favors P0 followed by body-aware P1 revision
over request-level retrieval followed by direct generation.

### Factor Comparisons

Aggregating both runs and judges gives:

| Comparison            | Left wins | Right wins | Both | Inconsistent |
| --------------------- | --------: | ---------: | ---: | -----------: |
| Direct × Request P1   |        15 |         44 |    0 |           13 |
| Direct × Direct Goal  |        30 |         27 |    0 |           15 |
| Request P1 × Goal P1  |        18 |         39 |    5 |           10 |
| Direct Goal × Goal P1 |        18 |         43 |    0 |           11 |
| Direct × Goal P1      |        20 |         44 |    0 |            8 |

The comparisons that hold retrieval evidence constant favor P1 revision:
Request P1 beats Direct 44–15, and Goal P1 beats Direct Goal 43–18. Direct
versus Direct Goal is nearly split at 30–27. The strongest observed factor is
therefore P0 anchoring plus revision, not request-level versus per-goal
retrieval by itself.

### Retrieval Noise And Threshold Sweep

Across the 36 evaluations in both runs, the authored cases contain 132 expected
skill occurrences. At the configured `0.30` threshold, the union of per-goal
retrieval selected 233 skill occurrences and recovered all 132 expected ones.
The remaining 101 selections show the size of the possible noise surface, but
they are not proven false positives because the authored expected-skill lists
are intentionally incomplete.

Recomputing selection from the persisted reranker traces gives this per-goal
union sweep:

| Threshold | Selected | Expected hits | Authored-set precision | Authored-set recall |
| --------: | -------: | ------------: | ---------------------: | ------------------: |
|     0.300 |      233 |           132 |                  56.7% |              100.0% |
|     0.325 |      199 |           129 |                  64.8% |               97.7% |
|     0.350 |      157 |           124 |                  79.0% |               93.9% |
|     0.375 |      135 |           115 |                  85.2% |               87.1% |
|     0.400 |      126 |           111 |                  88.1% |               84.1% |

Increasing the threshold removes many extra candidates, but it also loses
expected skills quickly. Threshold tuning is therefore a useful baseline, not
a sufficient filter. These results motivate a separate semantic gate that may
select no skill, removes irrelevant or redundant candidates by marginal
planning utility, and passes the unchanged full bodies of retained skills to
P1.

These are diagnostic results from six authored cases. Repeated rounds are not
independent tasks, judge outputs are model-based assessments, and authored
expected skills are incomplete relevance labels. The evidence does not support
a confirmatory general claim.

## Usage

Run with human evaluation:

```sh
npm run llm:direct-skill-planning
```

Run the blind LLM judge:

```sh
npm run llm:direct-skill-planning -- --judge
```

Run one named case:

```sh
npm run llm:direct-skill-planning -- --judge --case "production migration"
```

Each run is saved as an atomic JSON checkpoint below
`.llm-lab/direct-skill-planning/runs`. The file follows the
`scripts/full-skill-vs-hints` run envelope with an ID, status, timestamps,
configuration, and results. Each result contains P0, all four final plans, five
pairwise comparisons, and the retrieval trace. In judge mode, every comparison
is stored once per judge with the model identifier, a status, and its accepted
orientation judgments. If a judge orientation still fails after its available
attempts, its comparison is stored as `partial` or `failed` with only the safe
provider metadata and call context described above; successful orientations
are retained. The other judge and comparisons continue, and the run finishes
as `completed_with_failures`. P0 is retained only as diagnostic data and is
never sent to a judge or to the Direct Goal final planning call.

Every request and goal retrieval trace stores all three stages separately:

- `vectorShortlist`: all 20 Victor candidates with vector rank and score;
- `reranked`: the reranker's full top-ten result with relevance scores;
- `selected`: the candidates that also passed the minimum reranker score and
  were supplied to the planning call.

The command requires network access to the pinned SkillsBench raw files plus
`OPENROUTER_API_KEY`, and makes paid model and reranker calls. Configuration is
controlled by:

- `LLM_LAB_ROUNDS` (default `3`);
- `LLM_LAB_TOP_K` (default `10`, maximum `20`);
- `LLM_LAB_MIN_RERANKER_SCORE` (default `0.30`);
- `LLM_LAB_MODEL` (default `deepseek/deepseek-v4-pro`);
- `LLM_LAB_JUDGE_MODEL` (primary judge, default
  `google/gemini-3.7-flash`).

The secondary judge is fixed to `openai/gpt-5.6-sol` so every judge-mode
run produces both analyses.

Judge-mode checkpoints are written after each case-round evaluation finishes,
in task order, through a serialized atomic-write queue. If planning, embedding,
or reranking still fails after its retries, the run pauses, but every completed
evaluation remains in the checkpoint. The terminal also reports the safe
provider failure record instead of silently hiding the reason. The checkpoint
stores the final safe record, including HTTP status when supplied by the
provider, in its top-level `failure` field. Unexpected local failures use the
non-sensitive `unexpected_error` code.

Reasoning effort is a fixed experiment input:

- planning and goal decomposition: `high`;
- pairwise judge: `medium`.

The embedding and reranker models are fixed experiment inputs:

- embeddings: `voyageai/voyage-4-large`, 1024 dimensions;
- reranking: `voyageai/rerank-2.5`.

P0 and the base P1 review instruction reuse the applicable prompts from
`scripts/full-skill-vs-hints`. This experiment adds the same goal and relevance
contract to all four final policies. Direct Goal reuses the Direct prompt
unchanged and differs only in the retrieved skill set.

The terminal output follows `scripts/full-skill-vs-hints`: Clack progress,
blind options for human mode, per-round judgments, retrieval scores, and final
counts. Counts are reported independently for each judge and each of the five
pairwise comparisons.

In judge mode, every case and round runs concurrently. Within each evaluation,
independent planning calls, per-goal retrieval calls, all five comparisons,
and both model analyses also run concurrently once their inputs exist. Direct
Goal and Goal P1 await the same per-goal skill union. Human mode stays
sequential because every comparison requires an interactive terminal answer.

The authored local cases make this a diagnostic instrument. A confirmatory
claim requires a larger frozen held-out corpus and a predeclared analysis
threshold.
