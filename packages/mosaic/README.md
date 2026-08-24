# mosaic

Goal-oriented planning and preparation workflow extracted from the Doric host.

```ts
import mosaic, { mosaic as createMosaic } from 'mosaic';
```

The factory receives a provider, logger, independent planning, revision, and
execution profiles, reranker and embedder model IDs, skill and tool catalogs,
both retrievers, and explicit `routing.maxHintCandidates`,
`routing.maxRetrievedCandidates`, `routing.maxSkills`, `execution.maxTurns`,
and `revision.max` limits. The
per-node turn limit is required and accepts only positive safe integers. The
localized revision limit is required and accepts any integer greater than or
equal to zero; the Doric composition root uses `execution.maxTurns: 16` and
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
Every materialized graph carries a runtime-owned safe non-negative integer
`revision`. P0 is revision `0`, the single body-aware P1 is revision `1`, and
every successful localized revision increments it by one. `graphs[]` preserves
those contiguous snapshots in revision order, but array position is not the
revision authority.
The graph array is the complete workflow state. Each node owns its complete
`Observation[]`, `NodeOutcome | null`, and `RuntimeTermination | null`. Outstanding work, the
number of successful localized revisions, and IDs removed by earlier revisions are all
derived from graph contents instead of being reconciled with parallel
ledgers, queues, counters, or retired-ID collections.

## Structured output boundary

Every model-authored structured object is validated locally against its full
Zod contract before it can change a graph or node. P0, P1, each concurrent hint
candidate, bundle selection, and each localized revision run through a fresh
ephemeral `agent` with empty in-memory message, executable-tool, and tool-call
storages. Execution
keeps its specialized per-node agent because it also owns executable tools,
observation history, and `execution.maxTurns`.

Structured planning and revision operations expose the operation's direct
object schema through a reserved `submit_structured_output` terminal tool and
never send a provider-native `schema` field. Invalid or malformed arguments cannot
mutate state. The agent may provide bounded diagnostics and make two correction
attempts; the next invalid submission fails with `invalid_structured_output`.
Concrete invalid primitive or missing leaves are repaired transactionally:
only rejected paths are copied into the ephemeral prior candidate, valid fields
remain unchanged, and the composed object is revalidated. Root, collection,
malformed-JSON, and cross-field failures regenerate the whole object. Rejected
candidates are retained only for protocol replay and never enter Mosaic events,
logs, or correction feedback. The terminal tool is never executed, registered
as a Mosaic tool, or materialized as an `Observation`. Provider and transport
failures propagate without an infrastructure retry. `provider.rerank` remains
a direct provider operation.

## Graph planning and revision

The planner's structured output owns only `id`, `goal`, `doneWhen`,
`dependsOn`, and `deliver`. Mosaic rejects empty values, empty criteria,
duplicate or missing dependencies, cycles, non-terminal deliveries, unknown
fields, and graphs without a terminal deliverable. It then assigns `revision`, `pending`,
the array-order `index`, empty `candidates`, `tools`, `artifacts`, and
`observations`, plus null
`bundle`, `outcome`, and `termination` fields itself.
Planning treats a node as independent only when completing it cannot require
revising another node's choices. Constraints that jointly determine feasibility
remain together, and an upstream choice cannot complete before constraints that
could invalidate it are checked. Production and final semantic verification
remain in the same node unless verification is an independently requested
deliverable. Every semantic claim in a goal must appear explicitly in
`doneWhen`.

P0 never reads the catalog. For every P0 goal, P1 sends each required skill to
the hint extractor in configured order, followed by optional candidates
retrieved independently for that goal. Optional retrieval excludes required
and stale skills, deduplicates canonical names, and applies
`routing.maxHintCandidates` as `K_hint`; required skills neither consume this
limit nor depend on the retriever. A required-only catalog skips retrieval but
still runs each extraction. `hint.result` records empty and material
extractions, while only material hints reach the P1 revision prompt.
`retrieval.result` names only optional skills actually recovered. Later
execution routing uses the independent `routing.maxRetrievedCandidates` as
`K_retrieve`. Both retrieval paths reapply their bound after canonical
filtering and deduplication, so an over-returning retriever cannot exceed
either limit.

A `needs_revision` node stores the semantic request in its outcome and the
complete ordered observation set on the node. `schedule` detects those nodes and routes
to the dedicated `revision` state, which selects one in deterministic wave
order and performs one localized revision without catalog hints before
returning to `schedule`. The planner receives every queued observation as
fenced tool name, input, and output evidence; internal `callId` values are never
exposed. Completed nodes and other non-pending nodes are preserved exactly;
only the target and pending nodes may change. A retained target restarts as
pending with routing, outcome, termination, and partial artifacts cleared. The
prior snapshot retains the complete `needs_revision` outcome. IDs present in
older snapshots but absent from the active graph cannot
be reused. A localized response that changes none of the planner-owned `id`,
`goal`, `doneWhen`, `dependsOn`, or `deliver` fields in the revisable region is
rejected before a snapshot is appended and may use the existing
structured-output repair budget; it therefore does not consume the
localized-revision limit. The successful revision count is derived from the active revision as
`max(0, revision - 1)`;
exhaustion of `revision.max` preserves that outcome, adds a runtime-owned
`revision_limit` termination, and blocks the target without a provider call.

## Bundle routing

The bundle state builds one collision-safe Markdown context from the original
request, current node, ordered completion criteria, and transitive-ancestor
artifacts. It retrieves at most `maxRetrievedCandidates` routable skills, reranks their
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

Always-available skills form Doric's derived universal profile. They participate
in P1 hint extraction for every P0 goal, but remain excluded from routing
candidates, `bundle.skills`, and `maxSkills`, then are injected into every
execution system prompt in manifest order. They may refer only to base tools
and cannot expand the node tool menu. `tools.retriever` remains available to
other Mosaic policies but is not a tool router for this state.

## Execution

Execution creates one `agent` per ready node with isolated in-memory message
storage and executable tools resolved by name from the Mosaic tool catalog.
Each node resolves its selected skill names, then makes one `agent.complete`
call with the provider's tool definitions and a node-bound semantic decision
schema, bounded by `execution.maxTurns` provider invocations. Its collision-safe Markdown input
contains the original request, current goal and ordered completion criteria,
compact evidence from completed transitive ancestors only, ordered selected
skill bodies, and the available tool names and descriptions; tool schemas are
not duplicated in the prompt. Each projected ancestor includes its completed
status, criterion evidence and opaque observation IDs, total and cited
observation counts, one causally projected ledger containing each cited
observation's ID, producer, tool name, input, and output, and current artifacts.
References shared by criteria are rendered once in producer-ledger order.
Provider call IDs, uncited observations, and unrelated branches remain outside
the prompt. After a successful localized revision, each affected executor also
receives a short historical handoff containing the invalidated assumption,
requested effect, previously unsatisfied criteria, and relevant prior tool
results. The handoff omits observation and provider call IDs, is not citable,
and a post-revision `completed` decision must cite at least one fresh local
observation before its claims or results can prove a criterion. This additional
requirement does not apply to `blocked` or `failed`. Only local observations
cited by a previously unsatisfied criterion are eligible; at most the five
most recent are retained, oversized inputs and outputs are compacted, and the
omitted count is shown.

For every node, `agent` represents the decision schema as a strict terminal
tool. Provider requests therefore contain that terminal tool plus any ordinary
tools, without a provider-native structured-output field. The
terminal call is validated by `agent` and is not executed as a Mosaic tool.

The model decision contains only `status`, ordered criterion evaluations,
`result`, `revisionRequest`, and `reason`; each evaluation contains
`criterionIndex`, `satisfied`, model-authored `evidence`, and
`observationIds`. IDs must be unique within the criterion and identify the
smallest supporting subset of the current node's ledger or the cited ledger of
a completed transitive ancestor. An empty subset is valid when proof requires
no tool result. IDs from unknown observations, unrelated branches, descendants,
retired snapshots, or evidence not shown in the execution context are rejected.
Its status is `completed`, `needs_revision`, `blocked`, or `failed`. The runtime
records every successful executable-tool return directly in the node's isolated
`ToolCallStorage`. One run-local allocator creates a lowercase six-hex handle
from the first six hexadecimal characters of a fresh UUID. It is shared by all
concurrent nodes and reserves observations in every graph snapshot plus the
current wave. Collisions generate another UUID; 32 consecutive collisions fail
as an internal operational error. The resulting handle is reused unchanged in
tool-result messages, events, records, decisions, revisions, and results.
Evaluation-hook observations must use the same six-hex form and are claimed by
the shared allocator, which rejects malformed or already reserved IDs.
Mosaic creates ordered `Observation` records with those handles and rejects
unauthorized referenced IDs before storing the outcome or promoting artifacts.
`callId` remains correlation data on the full node observation but is not a
criterion reference. The terminal structured-output tool is never an
observation.

If a node uses its final permitted turn to request tools, those tools execute
and their results remain in the isolated history. Before another model call,
the agent raises `turn_limit_exceeded`; Mosaic materializes the complete ordered
observation ledger on the node and records a data-free runtime-owned
`turn_limit` termination.
The node becomes `blocked` and the concurrent wave resolves normally without
inventing a model decision or evidence.

The runtime materializes the node observation ledger before validating and
storing its semantic outcome. A completed decision is valid with zero, one, or
many observations; it stores its Markdown result as an inline `text/markdown` artifact,
appends additional artifacts, marks the node completed, and returns the fully
completed wave to `schedule`. A valid `needs_revision` decision requires at
least one observation, preserves no partial result, stores its semantic request
and keeps the complete observation set on the node, then returns normally so
graph-derived revision work can be processed. Model-authored `blocked` and
`failed` decisions resolve the wave normally, but `blocked` requires at least
one unsatisfied criterion. It does not universally require a local observation,
because a logical impossibility may be tool-free. Provider, tool, schema,
correlation, and state-machine failures still reject `prompt()` with their
original identity.
Before completing, the executor inspects available failure, timeout, stderr,
and truncation signals. A later successful command does not erase an earlier
failure without a fresh observation of the affected state. A semantic
`blocked` decision requires concrete impossibility evidence, reasonable
alternatives tried, and consideration of `needs_revision`; operational failures
remain separate runtime failures.
Execution logs node IDs, terminal statuses, and selected skill and tool names,
never goals, prompts, decisions, outcomes, reasons, tool payloads, or error
details.

New runs emit only MOSAIC event schema version 3. `tool.finished` includes the
same `observationId` stored in the node ledger. Doric persists these event
objects unchanged as JSONB; older stored event versions remain replayable and
require no database migration. IO capture redacts reserved terminal arguments
and rejected structured-response text; validated graphs, decisions, and
outcomes remain observable through their state events.

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
  readonly artifacts: readonly Artifact[];
  readonly observations: readonly Observation[];
}

interface Observation {
  readonly id: string;
  readonly goalId: string;
  readonly toolName: string;
  readonly callId: string;
  readonly input: string;
  readonly output: string;
}

interface WorkflowNodeResult {
  readonly id: string;
  readonly goal: string;
  readonly doneWhen: readonly string[];
  readonly status: 'completed' | 'blocked' | 'failed';
  readonly candidates: readonly SkillCandidate[];
  readonly bundle: OrderedBundle | null;
  readonly observations: readonly Observation[];
  readonly outcome: NodeOutcome | null;
  readonly termination: RuntimeTermination | null;
}

type Artifact =
  | { kind: 'inline'; mime: string; data: string }
  | { kind: 'reference'; mime: string; reference: string };

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
Artifact MIME types and opaque references are normalized non-empty strings;
inline data may be empty. Mosaic validates and transports references without
resolving, persisting, authorizing, or checking their targets. Artifact order
is preserved through outcomes, snapshots, causal projection, and delivery.
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
[LaTeX source](docs/revised/mosaic_0_2/mosaic_0_2.tex). The companion empirical
protocol 0.2 is historical and archived; it is not the active evaluation
surface. The current minimal public-benchmark comparison lives in
[`benchmarks/mosaic`](../../benchmarks/mosaic). The supplied MOSAIC 0.1 editions
remain unchanged under `docs/original/`.

| Runtime concept               | MOSAIC 0.2 definition                                                                                                                               |
| ----------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| Structured output conformance | Section 4.2 and C15: every model-authored structured object satisfies the complete runtime contract before changing state.                          |
| Ordered skill bundle          | Section 3.5: selected skills are unique, bounded, observably ordered, and may be empty.                                                             |
| Contextual skill routing      | Section 4.6: retrieval and reranking use the objective, criteria, original request, and relevant prior outputs.                                     |
| Exact tool menu               | Section 4.7: available tools are the base set plus tools declared by selected skills, without a separate tool router.                               |
| Projected node context        | Section 4.8: the executor receives compact causal ancestor criteria, cited observations, and current artifacts needed for the objective.            |
| Decision and evidence         | Sections 4.9-4.10: the model cites opaque observation IDs per criterion; the runtime validates them against local and causally projected ledgers.   |
| Localized revision            | Section 4.10: every observation from the requesting node is associated automatically and rendered without provider call IDs.                        |
| Scheduler/executor order      | Algorithm 1: ready nodes execute in waves, observations are appended in deterministic node and result order, and revisions preserve completed work. |
| Normative contracts           | Appendix A: `SkillCandidate`, `OrderedBundle`, `NodeDecision`, `Observation`, runtime `NodeOutcome`, and `RevisionRequest` are separate contracts.  |
| Final assembly                | Section 4.11: final assembly does not call the model or apply skills again.                                                                         |

The paper recommends a reserved terminal tool as its provider-independent
structured-output profile while allowing other protocols that preserve full
validation, atomicity, bounded feedback, and bounded recovery. Concurrent ready
waves, `Promise.allSettled`, the exact ephemeral-agent composition, explicit
per-criterion proof entries, and promotion of primary Markdown as an inline
`text/markdown` artifact are Doric choices.
Conforming implementations may vary in model, language, serialization format,
and tool-calling protocol while preserving the MOSAIC 0.2 invariants and
Appendix A contracts.

Current differences between the implementation and the broader paper contracts
are tracked in [MOSAIC 0.2 implementation gaps](docs/IMPLEMENTATION_GAPS.md).
