# MOSAIC 0.2 implementation gaps

Last reviewed: 2026-08-08

This document tracks differences between the current Doric implementation and
the broader MOSAIC 0.2 algorithm and normative contracts. The fourteen
mandatory conformance requirements in Appendix B are substantially covered;
the remaining work is concentrated in lifecycle semantics and the richer
contracts from Appendix A.

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

## High priority

1. **Complete the lifecycle semantics for `blocked` and `failed`.**
   The paper treats both statuses as terminal node outcomes. The current
   executor records the status and then fails the entire state machine, so
   independent branches cannot continue. Revision-limit exhaustion has the
   same behavior: it marks the node blocked and immediately fails the workflow.
   The runtime needs to retain terminal outcomes, continue deterministic work
   that is still eligible, and define the final workflow result when a required
   deliverable does not complete.

2. **Remove the all-nodes-completed assumption from successful termination.**
   Algorithm 1 runs until every node is terminal, while the current scheduler
   enters delivery only when every node is `completed`. Delivery also rejects
   any graph containing another terminal status. Termination should distinguish
   a completed delivery from a terminal workflow containing blocked or failed
   nodes instead of collapsing both into an exception.

3. **Expose a structured terminal workflow result.**
   `MosaicAgent.prompt` currently returns `FinalDelivery` or throws. It cannot
   distinguish a semantic block, a terminal node failure, a revision-limit
   block, and an infrastructure or state-machine error. A runtime-owned result
   contract should preserve those distinctions without weakening exact error
   propagation for operational failures.

## Medium priority

4. **Retain the complete runtime `NodeOutcome`.**
   Execution temporarily materializes the model decision plus all correlated
   observations, but then discards criterion evaluations and the terminal
   reason. The graph retains status, observations, revision request, and
   promoted artifacts only. Appendix A defines `NodeOutcome` as all fields of
   `NodeDecision` plus `observations[]`; that complete value should be a durable
   runtime contract available to later workflow policy and inspection.

5. **Add output schemas to runtime tool descriptors.**
   Appendix A requires each `ToolDescriptor` to expose both `inputSchema` and
   `outputSchema`. The current public tool definition contains only an input
   schema, and tool handlers return an unconstrained value. This gap concerns
   executable runtime tools, not test or validation tooling.

6. **Materialize the normative routing contracts.**
   The paper defines `SkillCandidate` with canonical name, score, rank, and
   rationale, and `OrderedBundle` with goal ID, ordered skills, and a selection
   rationale. The current implementation keeps ranking as private transient
   data and stores only selected skill names with per-skill rationales. Runtime
   behavior is deterministic, but the complete intermediate contracts are not
   available for inspection or downstream policy.

7. **Complete canonical skill-record normalization at the public boundary.**
   The bundle loader validates non-empty skill fields, but the public
   `SkillSchema` remains permissive, has no explicit `indexText`, and does not
   normalize duplicate `allowedTools`. Doric derives index text when populating
   embeddings, so the behavior exists implicitly rather than as the normative
   `SkillRecord` contract described in Appendix A.

## Low priority

8. **Make the plan revision explicit.**
   Appendix A includes `revision` in `Plan`. Mosaic currently infers it from
   the position of a graph snapshot in `graphs[]`. The behavior is stable, but
   the revision does not cross the plan boundary as an explicit field.

9. **Separate `K_hint` from `K_retrieve`.**
   Algorithm 1 exposes independent hint-retrieval and execution-retrieval
   limits. Mosaic uses one `routing.maxCandidates` value for both. Equal limits
   are valid, but callers cannot tune the two stages independently.

10. **Represent artifact references explicitly.**
    The normative `FinalDelivery` describes artifact references. Doric uses
    inline `{ mime, data }` artifacts, where `data` may contain either content
    or a reference without a distinct type. This is an intentional local
    representation, but it does not provide the explicit artifact-reference
    contract described by the paper.

## Suggested implementation order

1. Implement terminal workflow semantics for `blocked` and `failed`.
2. Retain and expose complete `NodeOutcome` values.
3. Add tool output schemas and strengthen catalog contracts.
4. Materialize routing contracts and explicit plan revisions.
5. Split retrieval limits and introduce explicit artifact references only when
   their additional policy value is needed.
