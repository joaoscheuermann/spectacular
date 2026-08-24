# MOSAIC 0.2 implementation gaps

Last reviewed: 2026-08-08

This document tracks differences between the current Doric implementation and
the broader MOSAIC 0.2 algorithm and normative contracts. The fifteen
mandatory conformance requirements in Appendix B are substantially covered;
gaps 1--7 are closed as of 2026-08-08.

The comparison uses the revised [MOSAIC 0.2 paper](revised/mosaic_0_2/mosaic_0_2.pdf),
especially sections 4.9-4.11, Algorithm 1, and Appendices A and B.

## Scope

This list excludes:

- empirical evaluation harnesses;
- tools whose sole purpose is testing or validating the agent;
- distributed persistence, transactional consistency, infrastructure retries,
  idempotency, complete observability, and infrastructure recovery; semantic
  structured-output validation and bounded correction remain core behavior;
- skill scripts, references, and assets that the paper explicitly leaves
  outside the core profile;
- more sophisticated tool filtering, authorization, or risk policies.

## Closed

- **Gap 1 — Runtime tool output schemas.** Executable factories expose Zod `input`
  and `output` schemas, descriptors materialize both JSON Schemas, and handler
  results are validated before execution resolves.
- **Gap 3 — Canonical skill-record normalization.** `SkillSchema` produces the same
  normalized `SkillRecord` consumed by the loader and Doric indexes its
  recalculated `indexText` directly.
- **Gap 2 — Normative routing contracts.** Nodes and public results expose
  strict `SkillCandidate` traces with exact reranker scores, contiguous ranks,
  and per-candidate rationales plus a strict `OrderedBundle` with the selected
  ordered subset and global selection rationale.
- **Gap 4 — Explicit runtime plan revision.** Every materialized graph carries
  a runtime-owned safe non-negative integer `revision`; P0 is `0`, P1 is `1`,
  localized revisions increment contiguously, and model-authored plans cannot
  set it.
- **Gap 5 — Independent retrieval limits.** `routing.maxHintCandidates`
  implements `K_hint` and `routing.maxRetrievedCandidates` implements
  `K_retrieve`; both have defensive post-normalization bounds, and `maxSkills`
  is validated only against the latter.
- **Gap 6 — Discriminated artifacts.** Outcomes, graph state, causal projection,
  and delivery use the strict public `Artifact` union between inline content and
  opaque references. The core validates and transports references but does not
  resolve, persist, authorize, or check them.
- **Gap 7 — Complete structured-output conformance.** P0, P1, every hint,
  bundle selection, node decision, and localized revision terminate through an
  agent-owned reserved tool. The original Zod schema validates arguments
  locally before state mutation, with atomic rejection, bounded diagnostics,
  two correction attempts, and `invalid_structured_output` on the third
  invalid submission. Provider-native structured output is not used by Mosaic.
