# MOSAIC 0.2 implementation gaps

Last reviewed: 2026-08-08

This document tracks differences between the current Doric implementation and
the broader MOSAIC 0.2 algorithm and normative contracts. The fourteen
mandatory conformance requirements in Appendix B are substantially covered;
the remaining open work concerns plan and artifact contracts from Appendix A.
Catalog gaps 1 and 3 and routing gap 2 were closed on 2026-08-08.

The comparison uses the revised [MOSAIC 0.2 paper](revised/mosaic_0_2/mosaic_0_2.pdf),
especially sections 4.8-4.10, Algorithm 1, and Appendices A and B.

## Scope

This list excludes:

- empirical evaluation harnesses;
- tools whose sole purpose is testing or validating the agent;
- distributed persistence, transactional consistency, infrastructure retries,
  idempotency, complete observability, and infrastructure recovery;
- skill scripts, references, and assets that the paper explicitly leaves
  outside the core profile;
- more sophisticated tool filtering, authorization, or risk policies.

## Low priority

4. **Make the plan revision explicit.**
   Appendix A includes `revision` in `Plan`. Mosaic currently infers it from
   the position of a graph snapshot in `graphs[]`. The behavior is stable, but
   the revision does not cross the plan boundary as an explicit field.

5. **Separate `K_hint` from `K_retrieve`.**
   Algorithm 1 exposes independent hint-retrieval and execution-retrieval
   limits. Mosaic uses one `routing.maxCandidates` value for both. Equal limits
   are valid, but callers cannot tune the two stages independently.

6. **Represent artifact references explicitly.**
   The normative `FinalDelivery` describes artifact references. Doric uses
   inline `{ mime, data }` artifacts, where `data` may contain either content
   or a reference without a distinct type. This is an intentional local
   representation, but it does not provide the explicit artifact-reference
   contract described by the paper.

## Suggested implementation order

1. Materialize explicit plan revisions.
2. Split retrieval limits and introduce explicit artifact references only when
   their additional policy value is needed.

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
