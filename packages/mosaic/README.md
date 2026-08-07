# mosaic

Goal-oriented planning and preparation workflow extracted from the Doric host.

```ts
import mosaic, { mosaic as createMosaic } from 'mosaic';
```

The factory receives provider, logger, default/reranker/embedder model IDs,
skill and tool catalogs, both vector databases, and explicit
`routing.maxCandidates`, `routing.maxSkills`, and `revision.max` limits. The
localized revision limit is required and accepts any integer greater than or
equal to zero; the Doric composition root uses `revision.max: 3`. It has no
session option. The embedder configuration and tool vectors remain part of the
public composition contract even though bundle routing queries only skill
vectors.

## Lifecycle

Mosaic owns the workflow policy and uses `state-machine` only for reusable,
run-local typed transition execution. Its fixed transitions are
`plan(P0) -> plan(P1) -> schedule -> bundle -> execution -> schedule` and
`schedule -> revision -> schedule`. The `plan` state cannot be re-entered
after P1. An already-completed graph finishes successfully, while a graph that
cannot produce another ready node preserves the intentional missing-ready
domain failure. The current workflow stores graphs as a run-local LIFO stack,
appends planned and revised graphs, and mutates the active graph at the end of
the stack when scheduling nodes.
`graphs[n]` is revision `n`, so P0 is `graphs[0]`, the single body-aware P1 is
`graphs[1]`, and every successful localized revision appends one later entry.
The graph array is the complete workflow state. Each node owns its correlated
observations and optional revision request. Outstanding work, the number of
successful localized revisions, and IDs removed by earlier revisions are all
derived from the graph snapshots instead of being reconciled with parallel
ledgers, queues, counters, or retired-ID collections.

## Graph planning and revision

The planner's structured output owns only `id`, `goal`, `doneWhen`,
`dependsOn`, and `deliver`. Mosaic rejects empty values, empty criteria,
duplicate or missing dependencies, cycles, non-terminal deliveries, unknown
fields, and graphs without a terminal deliverable. It then assigns `pending`,
the array-order `index`, empty `skills`, `tools`, `artifacts`, and `observations`,
and a null `revisionRequest` itself.

P0 never reads the catalog. P1 retrieves candidates independently for each
goal, excludes required and stale skills, deduplicates canonical names, and
applies `routing.maxCandidates` both as `K_hint` and as the later routing
`K_retrieve`. The hint path reapplies the bound after canonical filtering so an
over-returning vector index cannot cause more than `K_hint` model calls.

A `needs_revision` outcome stores the semantic request and the complete ordered
observation set produced by its node. `schedule` detects those nodes and routes
to the dedicated `revision` state, which selects one in deterministic wave
order and performs one localized revision without catalog hints before
returning to `schedule`. The planner receives every queued observation as
fenced tool name, input, and output evidence; internal `callId` values are never
exposed. Completed nodes and other non-pending nodes are preserved exactly;
only the target and pending nodes may change. A retained target restarts as
pending with routing, observations, its consumed request, and partial artifacts
cleared. IDs present in older snapshots but absent from the active graph cannot
be reused. The successful revision count is `max(0, graphs.length - 2)`;
exhaustion of `revision.max` blocks the target without a provider call.

## Bundle routing

The bundle state builds one collision-safe Markdown context from the original
request, current node, ordered completion criteria, and transitive-ancestor
artifacts. It retrieves at most `maxCandidates` routable skills, reranks their
complete canonical bodies, and validates at most `maxSkills` selections. Empty
selection is a normal result and skips reranking and model selection when no
candidates exist or `maxSkills` is zero.

Reranker responses must cover every candidate exactly once. Mosaic sorts them
locally by descending relevance score and canonical skill name for ties. The
model chooses a set, while Mosaic reconstructs the authoritative order from
that sorted list.

Nodes store no duplicated skill definitions. Their runtime-owned `skills`
array contains ordered references:

```ts
interface NodeSkillSelection {
  readonly skill: string;
  readonly rationale: string;
}
```

This Doric profile refines the paper's bundle-level selection rationale into a
rationale attached to each selected skill. An empty array is the canonical
representation of selecting no skill.

The runtime resolves each name against the canonical catalog when composing
tools and execution context. Base tools appear first, followed by tools declared
by selected skills in bundle order; the first occurrence of a name wins.

Always-available skills form Doric's derived universal profile. They are
excluded from hints, vector routing, `node.skills`, and `maxSkills`, then
injected into every execution system prompt in manifest order. They may refer
only to base tools and cannot expand the node tool menu. `tools.embeddings`
remains available to other Mosaic policies but is not a tool router for this
state.

## Execution

Execution creates one `agent` per ready node with isolated in-memory message
storage and executable tools resolved by name from the Mosaic tool catalog.
Each node resolves its selected skill names, then makes one `agent.complete`
call with the provider's tool definitions and a node-bound semantic decision
schema. Its collision-safe Markdown input
contains the original request, current goal and ordered completion criteria,
artifacts from transitive ancestor nodes only, ordered selected skill bodies,
and the available tool names and descriptions; tool schemas are not duplicated
in the prompt.

For nodes with executable tools, `agent` represents the decision schema as a
strict terminal tool. Provider requests therefore contain ordinary tools plus
that terminal tool, without a provider-native structured-output field. The
terminal call is validated by `agent` and is not executed as a Mosaic tool.

The model decision contains only `status`, ordered criterion evaluations,
`result`, `revisionRequest`, and `reason`; criterion `evidence` remains
model-authored prose. Its status is `completed`, `needs_revision`, `blocked`, or
`failed`. The runtime then correlates every successful executable-tool return
from the node's isolated message history and creates ordered `Observation`
records. `callId` exists only on those internal records for correlation. The
terminal structured-output tool is never an observation, and missing,
duplicate, or uncorrelated calls or results fail execution.

The runtime materializes the node outcome from the semantic decision and every
correlated observation. A completed decision is valid with zero, one, or many
observations; it stores its Markdown result as a `text/markdown` artifact,
appends additional artifacts, marks the node completed, and returns the fully
completed wave to `schedule`. A valid `needs_revision` decision requires at
least one observation, preserves no partial result, stores its semantic request
and the complete observation set on the node, and returns normally so
graph-derived revision work can be processed. `blocked`, `failed`, provider,
tool, schema, and observation-validation failures retain failure precedence.
Execution logs node IDs, terminal statuses, and selected skill and tool names,
never goals, prompts, decisions, outcomes, reasons, tool payloads, or error
details.

## Correspondence with the MOSAIC paper

The normative architecture is MOSAIC 0.2, available as the revised
[PDF](docs/revised/mosaic_0_2/mosaic_0_2.pdf) and editable
[LaTeX source](docs/revised/mosaic_0_2/mosaic_0_2.tex). Its companion empirical
validation protocol is also available as a revised
[PDF](docs/revised/mosaic_validation_protocol_0_2/mosaic_validation_protocol_0_2.pdf)
and [LaTeX source](docs/revised/mosaic_validation_protocol_0_2/mosaic_validation_protocol_0_2.tex).
The supplied MOSAIC 0.1 editions remain unchanged under `docs/original/`.

| Runtime concept          | MOSAIC 0.2 definition                                                                                                                               |
| ------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| Ordered skill bundle     | Section 3.5: selected skills are unique, bounded, observably ordered, and may be empty.                                                             |
| Contextual skill routing | Section 4.5: retrieval and reranking use the objective, criteria, original request, and relevant prior outputs.                                     |
| Exact tool menu          | Section 4.6: available tools are the base set plus tools declared by selected skills, without a separate tool router.                               |
| Projected node context   | Section 4.7: the executor receives only the context needed for the current objective.                                                               |
| Decision and evidence    | Sections 4.8-4.9: the model authors the semantic decision; the runtime creates ordered observations and materializes the node outcome.              |
| Localized revision       | Section 4.9: every observation from the requesting node is associated automatically and rendered without provider call IDs.                         |
| Scheduler/executor order | Algorithm 1: ready nodes execute in waves, observations are appended in deterministic node and result order, and revisions preserve completed work. |
| Normative contracts      | Appendix A: `NodeDecision`, `Observation`, runtime `NodeOutcome`, and `RevisionRequest` are separate contracts.                                     |
| Final assembly           | Section 4.10: final assembly does not call the model or apply skills again.                                                                         |

The paper specifies semantic contracts, not provider transport or dispatch
mechanics. Concurrent ready waves, `Promise.allSettled`, one isolated `agent`
instance per node, the strict terminal-output tool, explicit per-criterion proof
entries, and the `text/markdown` artifact representation are Doric choices.
Conforming implementations may vary in model, language, serialization format,
and tool-calling protocol while preserving the MOSAIC 0.2 invariants and
Appendix A contracts.
