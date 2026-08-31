# Skill Retrieval Gating

This diagnostic tests whether a semantic post-retrieval gate can remove
recovered skill noise while retaining relevant skills. It runs every case in
parallel through one fixed pipeline:

1. generate P0;
2. run lexical and vector retrieval, fuse their rankings, and rerank skills
   independently for every P0 goal using `Objective + Goal`;
3. classify every retrieved `Goal + Skill` pair as `keep` or `drop` with a
   reason;
4. generate P1 from the kept skill bodies; and
5. evaluate the final bundle against the case's exhaustive skill labels.

Cases are JSON files in `cases/`. Their `skills.expected` lists required useful
skills, `skills.useful` lists acceptable supplementary skills, and
`skills.noise` maps every remaining skill name to one of `irrelevant`,
`no_operational_value`, or `conflicting`. Together they must partition the
complete local catalog. These labels are used only for offline evaluation and
are never passed to retrieval, the gate, or planning. Skill bodies are
preloaded Markdown files in `cases/skills/`; the runtime never downloads
skills.

A skill reaches the final bundle when at least one goal keeps it. Skill names
are deduplicated before P1. The fixed per-goal retrieval configuration is
`retrievalK = 20`, `minVectorScore = 0.3`, and `topK = 10`. The BM25 lexical
index searches the canonical skill name and complete body. Vector candidates
below the inclusive minimum score are removed from the semantic ranking. The
lexical and retained semantic rankings are fused through Victor's equal-weight
reciprocal rank fusion, bounded to 20 candidates, before the top 10 are chosen
by the reranker. The recovered bundle is the unique union of those per-goal
results before gating. Per-case and micro-aggregate metrics are:

```text
recall                = selected expected / all expected
noiseRate             = selected noise / all selected
precision             = selected relevant / all selected
F1                    = 2 * precision * recall / (precision + recall)
noiseRemovalRate      = removed recovered noise / all recovered noise
relevantRetentionRate = selected relevant / all recovered relevant
selectionF1           = 2 * precision * relevantRetentionRate
                        / (precision + relevantRetentionRate)
```

Relevant means `expected` or `useful`. Noise removal and relevant retention
compare the recovered bundle before gating with the selected bundle after
gating. F1 combines precision with required-skill recall; selection F1 combines
precision with retention of all recovered relevant skills. A zero denominator
produces zero. Set `OPENROUTER_API_KEY` and run:

```sh
npm run llm:skill-retrieval-gating
```

Every pipeline action uses up to five total attempts with exponential backoff.
The waits before attempts two through five are 15, 30, 60, and 120 seconds.
This step-level retry applies to any thrown failure, including provider errors
marked non-retryable. A failed indexing attempt is discarded and rebuilt from
a fresh local index before retrying. The final failure is rethrown unchanged
after the fifth attempt.

Pipeline logic lives in `index.mjs`, run identity and output persistence in
`output.mjs`, prompt text in `prompt.mjs`, message construction in `utils.mjs`,
and Zod contracts in `schemas.mjs`. Every invocation creates an ignored
`output/<run-id>/` directory containing:

- `manifest.json`: status and timestamps, the effective configuration, Git
  commit and dirty-worktree flag, hashes and counts for the cases and skill
  catalog, a hash covering the experiment sources and package metadata, and
  the Node.js environment;
- `output.log`: all Pino progress, intermediate results, final bundles, plans,
  provider call usage, and metrics as JSON Lines; and
- `results.json`: the complete per-case results and aggregate metrics after a
  successful run.

The manifest deliberately excludes credentials. The local `.gitignore` keeps
the complete `output/` tree and the former root-level `output.log` out of Git.

Every per-goal retrieval result preserves the lexical ranking, all vector
candidates and their pre-threshold cosine scores, the vector candidates removed
by the threshold, the fused hybrid ranking, and the reranker input/output
mapping. Reranker inputs store the exact query plus catalog-bound candidate
names, positions, lexical, vector, and hybrid ranks and scores; skill bodies
remain identified by the catalog hash instead of being duplicated in every
trace. Successful provider calls log their reported usage individually.
Completed and failed manifests, and successful `results.json` files, include
aggregate token, search-unit, and cost totals from resolved calls. OpenRouter
cost uses credits; absent provider accounting remains distinguishable through
resolved-call counts.

## Related work

This diagnostic adapts post-retrieval context filtering to complete agent
skills. The comparison is analogous rather than exact: a retrieved context or
tool corresponds to a recovered skill, relevance acceptance corresponds to the
`keep | drop` gate, and the accepted context set corresponds to the final skill
bundle. The current metrics evaluate selection quality; they do not establish
that P1 generated from the filtered bundle is better than P1 generated from all
recovered skills.

For the recovered-skill population, the binary-classification interpretation
is `TP = selectedExpected + selectedUseful`, `FP = selectedNoise`,
`FN = recoveredRelevant - TP`, and `TN = removedNoise`. Consequently,
`noiseRemovalRate` is specificity, `relevantRetentionRate` is sensitivity, and
`selectionF1` is the positive-class F1. The separate `recall` metric measures
coverage of required `expected` skills rather than sensitivity over every
recovered relevant skill.

### Context filtering and noise robustness

- [RE-RAG: Improving Open-Domain QA Performance and Interpretability with Relevance Estimator in Retrieval-Augmented Generation](https://aclanthology.org/2024.emnlp-main.1236/)
  (EMNLP 2024) classifies whether each retrieved context is useful. Its
  per-query, per-context relevance estimator is the closest analogue to this
  diagnostic's pairwise gate.
- [Learning to Filter Context for Retrieval-Augmented Generation (FILCO)](https://arxiv.org/abs/2311.08377)
  filters retrieved content before generation. It operates on finer-grained
  spans rather than complete skills.
- [Making Retrieval-Augmented Language Models Robust to Irrelevant Context](https://proceedings.iclr.cc/paper_files/paper/2024/hash/8011b23e1dc3f57e1b6211ccad498919-Abstract-Conference.html)
  (ICLR 2024) studies the same central trade-off: filtering irrelevant context
  can prevent degradation while also discarding relevant evidence.
- [RECOMP: Improving Retrieval-Augmented LMs with Context Compression and Selective Augmentation](https://proceedings.iclr.cc/paper_files/paper/2024/hash/bda88ed2892f5e61c9a9bf215c566913-Abstract-Conference.html)
  (ICLR 2024) permits an empty compression when retrieved material is
  irrelevant or unhelpful, analogous to dropping candidates before generation.
- [Self-RAG: Learning to Retrieve, Generate, and Critique through Self-Reflection](https://proceedings.iclr.cc/paper_files/paper/2024/hash/25f7be9694d7b32d5cc670927b8091e1-Abstract-Conference.html)
  (ICLR 2024) evaluates passage relevance and whether retrieval is useful,
  although its decisions are integrated into generation rather than applied as
  this diagnostic's independent post-retrieval gate.
- [An Information Bottleneck Perspective for Effective Noise Filtering on Retrieval-Augmented Generation](https://aclanthology.org/2024.acl-long.59/)
  (ACL 2024) jointly targets removal of retrieved noise and preservation of
  information useful to the output, motivating measurement of both noise
  removal and relevant retention.
- [The Power of Noise: Redefining Retrieval for RAG Systems](https://doi.org/10.1145/3626772.3657834)
  (SIGIR 2024) shows that the effect of irrelevant context depends on the kind
  of noise; high-ranked but unhelpful passages can be more harmful than random
  passages.
- [The Distracting Effect: Understanding Irrelevant Passages in RAG](https://aclanthology.org/2025.acl-long.892/)
  (ACL 2025) distinguishes benign irrelevant passages from hard distractors,
  supporting future cases with semantically close, plausible noise.

### Retrieval evaluation

- [RAGChecker: A Fine-grained Framework for Diagnosing Retrieval-Augmented Generation](https://proceedings.neurips.cc/paper_files/paper/2024/hash/27245589131d17368cccdfa990cbf16e-Abstract-Datasets_and_Benchmarks_Track.html)
  (NeurIPS 2024) separates retrieval and generation diagnostics, matching the
  decision to evaluate bundle selection independently from downstream P1
  quality.
- [RAGAs: Automated Evaluation of Retrieval Augmented Generation](https://aclanthology.org/2024.eacl-demo.16/)
  (EACL 2024) includes focused-context relevance among distinct retrieval and
  generation dimensions. This diagnostic instead uses exhaustive human-authored
  gold labels.
- [Benchmarking Large Language Models in Retrieval-Augmented Generation (RGB)](https://ojs.aaai.org/index.php/AAAI/article/view/29728)
  (AAAI 2024) explicitly evaluates noise robustness and negative rejection.
- [“Knowing When You Don't Know”: A Multilingual Relevance Assessment Dataset for Robust Retrieval-Augmented Generation (NoMIRACL)](https://aclanthology.org/2024.findings-emnlp.730/)
  (Findings of EMNLP 2024) evaluates relevant and non-relevant subsets
  separately and exposes the trade-off between accepting noise and rejecting
  useful evidence.

### Tool and skill retrieval

- [ToolRerank: Adaptive and Hierarchy-Aware Reranking for Tool Retrieval](https://aclanthology.org/2024.lrec-main.1413/)
  (LREC-COLING 2024) evaluates a retrieve-then-rerank pipeline for selecting
  tools from larger catalogs.
- [Task-Aligned Tool Recommendation for Large Language Models](https://aclanthology.org/2025.ijcnlp-long.110/)
  (IJCNLP-AACL 2025) targets a precise task-specific tool set and penalizes both
  missing and extraneous tools, closely matching final-bundle evaluation.
- [Retrieval Models Aren't Tool-Savvy: Benchmarking Tool Retrieval for Large Language Models](https://aclanthology.org/2025.findings-acl.1258/)
  (Findings of ACL 2025) provides a large-scale tool retrieval benchmark and
  connects retrieval quality with downstream task success.
- [Do LLMs Know Tool Irrelevance? Demystifying Structural Alignment Bias in Tool Invocations](https://aclanthology.org/2026.acl-long.1473/)
  (ACL 2026) shows why structurally applicable but semantically irrelevant tools
  are valuable hard negatives for this benchmark.
- [SkillRouter: Skill Routing for LLM Agents at Scale](https://arxiv.org/abs/2603.22455)
  (2026 preprint) evaluates body-aware retrieve-and-rerank routing, supporting
  this diagnostic's use of complete skill bodies during gating.
- [SkillRet: A Large-Scale Benchmark for Skill Retrieval in LLM Agents](https://arxiv.org/abs/2605.05726)
  (2026 preprint) studies retrieval over a large, noisy skill library and offers
  a scaling reference beyond this diagnostic's six cases.
- [Skill Is Not Document: A Query-Conditional Benchmark and Two-Stage Retriever for LLM Agent Skill Routing](https://arxiv.org/abs/2606.03565)
  (2026 preprint) models compatibility among jointly selected skills. It also
  highlights what this pairwise gate does not measure: bundle-level
  compatibility and redundancy.
- [How Well Do Agentic Skills Work in the Wild: Benchmarking LLM Skill Usage in Realistic Settings](https://arxiv.org/abs/2604.04323)
  (2026 preprint) evaluates whether retrieved skills improve end-task success,
  corresponding to the deferred comparison between filtered and unfiltered P1
  plans.
