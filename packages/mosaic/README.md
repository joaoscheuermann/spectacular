# mosaic

Goal-oriented planning and preparation workflow extracted from the Doric host.

```ts
import mosaic, { mosaic as createMosaic } from 'mosaic';
```

The factory receives a provider, logger, default/reranker model IDs, skill and
tool catalogs, both retrievers, and explicit `routing.maxCandidates`,
`routing.maxSkills`, `execution.maxTurns`, and `revision.max` limits. The
per-node turn limit is required and accepts only positive safe integers. The
localized revision limit is required and accepts any integer greater than or
equal to zero; the Doric composition root uses `execution.maxTurns: 8` and
`revision.max: 3`. It has no session option. The tool
retriever remains part of the public composition contract even though bundle
routing queries only the skill retriever.

## Lifecycle

Mosaic owns the workflow policy and uses `state-machine` only for reusable,
run-local typed transition execution. Its fixed transitions are
`plan(P0) -> plan(P1) -> schedule -> bundle -> execution -> schedule -> delivery -> finish(MosaicResult)` and
`schedule -> revision -> schedule`. The `plan` state cannot be re-entered
after P1. An already-terminal graph transitions to deterministic finalization,
while a graph that
cannot produce another ready node preserves the intentional missing-ready
domain failure. The current workflow stores graphs as a run-local LIFO stack,
appends planned and revised graphs, and mutates the active graph at the end of
the stack when scheduling nodes.
`graphs[n]` is revision `n`, so P0 is `graphs[0]`, the single body-aware P1 is
`graphs[1]`, and every successful localized revision appends one later entry.
The graph array is the complete workflow state. Each node owns a complete
`NodeOutcome | null` and `RuntimeTermination | null`. Outstanding work, the
number of successful localized revisions, and IDs removed by earlier revisions are all
derived from the graph snapshots instead of being reconciled with parallel
ledgers, queues, counters, or retired-ID collections.

## Graph planning and revision

The planner's structured output owns only `id`, `goal`, `doneWhen`,
`dependsOn`, and `deliver`. Mosaic rejects empty values, empty criteria,
duplicate or missing dependencies, cycles, non-terminal deliveries, unknown
fields, and graphs without a terminal deliverable. It then assigns `pending`,
the array-order `index`, empty `candidates`, `tools`, and `artifacts`, plus null
`bundle`, `outcome`, and `termination` fields itself.

P0 never reads the catalog. P1 retrieves candidates independently for each
goal, excludes required and stale skills, deduplicates canonical names, and
applies `routing.maxCandidates` both as `K_hint` and as the later routing
`K_retrieve`. The hint path reapplies the bound after canonical filtering so an
over-returning retriever cannot cause more than `K_hint` model calls.

A `needs_revision` outcome stores the semantic request and the complete ordered
observation set produced by its node. `schedule` detects those nodes and routes
to the dedicated `revision` state, which selects one in deterministic wave
order and performs one localized revision without catalog hints before
returning to `schedule`. The planner receives every queued observation as
fenced tool name, input, and output evidence; internal `callId` values are never
exposed. Completed nodes and other non-pending nodes are preserved exactly;
only the target and pending nodes may change. A retained target restarts as
pending with routing, outcome, termination, and partial artifacts cleared. The
prior snapshot retains the complete `needs_revision` outcome. IDs present in
older snapshots but absent from the active graph cannot
be reused. The successful revision count is `max(0, graphs.length - 2)`;
exhaustion of `revision.max` preserves that outcome, adds a runtime-owned
`revision_limit` termination, and blocks the target without a provider call.

## Bundle routing

The bundle state builds one collision-safe Markdown context from the original
request, current node, ordered completion criteria, and transitive-ancestor
artifacts. It retrieves at most `maxCandidates` routable skills, reranks their
complete canonical bodies, and validates at most `maxSkills` selections. Empty
selection is a normal result. When no candidates exist, Mosaic materializes a
deterministic empty bundle without reranking or selection. When `maxSkills` is
zero, it also skips retrieval and materializes a distinct deterministic trace.

Reranker responses must cover every candidate exactly once. Mosaic sorts them
locally by descending relevance score and canonical skill name for ties while
preserving each exact finite provider score. The selector evaluates every
candidate exactly once, including rejected candidates, and supplies one global
selection rationale. Mosaic reconstructs the selected bundle in authoritative
reranker order.

Nodes store no duplicated skill definitions. Their runtime-owned routing trace
uses the exported strict contracts:

```ts
interface SkillCandidate {
  readonly skillName: string;
  readonly score: number;
  readonly rank: number;
  readonly rationale: string;
}

interface OrderedBundle {
  readonly goalId: string;
  readonly skills: readonly string[];
  readonly selectionRationale: string;
}
```

Candidate ranks are contiguous and one-based. Names are unique, rationales are
trimmed non-empty text of at most 500 characters, and bundle skills are a
unique ordered subset of candidate names. `bundle: null` means exclusively that
the node has never passed routing; a routed empty bundle is a non-null object
whose `skills` array is empty. The same trace is copied into
`WorkflowNodeResult`.

The runtime resolves names exclusively from `bundle.skills` against the
canonical catalog when composing tools and execution context. Candidate and
selection rationales remain inspectable diagnostics and are never execution
instructions. Base tools appear first, followed by tools declared by bundle
skills in order; the first occurrence of a name wins.

Bundle skills are canonical `SkillRecord` values. The bundle boundary trims
required text, removes duplicate `allowedTools` while preserving first
occurrence, and always recalculates `indexText` from the normalized record.
Doric uses that same `indexText` directly for lexical and vector indexing.

Always-available skills form Doric's derived universal profile. They are
excluded from hints, routing candidates, `bundle.skills`, and `maxSkills`, then
injected into every execution system prompt in manifest order. They may refer
only to base tools and cannot expand the node tool menu. `tools.retriever`
remains available to other Mosaic policies but is not a tool router for this
state.

## Execution

Execution creates one `agent` per ready node with isolated in-memory message
storage and executable tools resolved by name from the Mosaic tool catalog.
Each node resolves its selected skill names, then makes one `agent.complete`
call with the provider's tool definitions and a node-bound semantic decision
schema, bounded by `execution.maxTurns` provider invocations. Its collision-safe Markdown input
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

If a node uses its final permitted turn to request tools, those tools execute
and their results remain in the isolated history. Before another model call,
the agent raises `turn_limit_exceeded`; Mosaic materializes the complete ordered
observation ledger and records it in a runtime-owned `turn_limit` termination.
The node becomes `blocked` and the concurrent wave resolves normally without
inventing a model decision or evidence.

The runtime materializes and durably stores the node outcome from the semantic
decision and every correlated observation. A completed decision is valid with zero, one, or many
observations; it stores its Markdown result as a `text/markdown` artifact,
appends additional artifacts, marks the node completed, and returns the fully
completed wave to `schedule`. A valid `needs_revision` decision requires at
least one observation, preserves no partial result, stores its semantic request
and the complete observation set on the node, and returns normally so
graph-derived revision work can be processed. Model-authored `blocked` and
`failed` decisions resolve the wave normally. Provider, tool, schema,
correlation, and state-machine failures still reject `prompt()` with their
original identity.
Execution logs node IDs, terminal statuses, and selected skill and tool names,
never goals, prompts, decisions, outcomes, reasons, tool payloads, or error
details.

## Final delivery

`MosaicAgent.prompt(input)` resolves to a `MosaicResult`. The scheduler runs
until every node is terminal. A pending descendant of a blocked or failed node
is blocked with a runtime-owned `dependency` termination naming its direct
non-completed dependencies, while independent branches continue. Finalization
makes no model, provider, skill, or tool calls and orders every node
topologically with original node-array position as the tie-breaker.

```ts
interface FinalDelivery {
  readonly markdown: string;
  readonly parts: readonly FinalDeliveryPart[];
}

interface FinalDeliveryPart {
  readonly id: string;
  readonly goal: string;
  readonly markdown: string;
  readonly artifacts: readonly DeliveryArtifact[];
  readonly observations: readonly Observation[];
}

type MosaicResult =
  | {
      status: 'completed';
      delivery: FinalDelivery;
      nodes: WorkflowNodeResult[];
    }
  | { status: 'blocked' | 'failed'; nodes: WorkflowNodeResult[] };
```

Only a workflow whose nodes all completed includes `delivery`. Part Markdown is
retained verbatim and `markdown` joins it using exactly `\n\n`. The primary
Markdown artifact is not duplicated in `artifacts`.
Artifacts and observations are copies; observations intentionally expose the
complete ordered runtime ledger, including call IDs, tool inputs, and outputs.
The strict candidate, ordered-bundle, outcome, termination, node-result,
workflow-result, and delivery schemas are exported from `mosaic`. Doric prints
only completed Markdown, with
one terminal newline when needed; blocked and failed results emit only a safe
status-and-node-ID log.

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
| Normative contracts      | Appendix A: `SkillCandidate`, `OrderedBundle`, `NodeDecision`, `Observation`, runtime `NodeOutcome`, and `RevisionRequest` are separate contracts.  |
| Final assembly           | Section 4.10: final assembly does not call the model or apply skills again.                                                                         |

The paper specifies semantic contracts, not provider transport or dispatch
mechanics. Concurrent ready waves, `Promise.allSettled`, one isolated `agent`
instance per node, the strict terminal-output tool, explicit per-criterion proof
entries, and the `text/markdown` artifact representation are Doric choices.
Conforming implementations may vary in model, language, serialization format,
and tool-calling protocol while preserving the MOSAIC 0.2 invariants and
Appendix A contracts.

Current differences between the implementation and the broader paper contracts
are tracked in [MOSAIC 0.2 implementation gaps](docs/IMPLEMENTATION_GAPS.md).
