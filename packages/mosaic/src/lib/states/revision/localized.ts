import {
  GraphSchema,
  type PlannedGraph,
  PlannedGraphSchema,
} from '../../schemas/graph.js';
import type { Graph, Node } from '../../types/graph.js';

/** Returns successful localized revisions already represented by graph snapshots. */
export const localizedRevisionCount = (graphs: readonly Graph[]): number =>
  // Revisions after P1 consume R_max; the active revision is authoritative.
  Math.max(0, (graphs.at(-1)?.revision ?? 1) - 1);

/** Returns IDs present in history but absent from the active graph, in stable order. */
export const retiredNodeIds = (graphs: readonly Graph[]): string[] => {
  // Without an active snapshot there is no meaningful retirement comparison.
  const active = graphs.at(-1);

  if (active === undefined) {return [];}

  // Any historical ID absent from the active graph is retired exactly once.
  const activeIds = new Set(active.nodes.map(({ id }) => id));
  const seen = new Set<string>();

  return graphs.slice(0, -1).flatMap((graph) =>
    graph.nodes.flatMap(({ id }) => {
      if (activeIds.has(id) || seen.has(id)) {return [];}

      seen.add(id);

      return [id];
    }),
  );
};

/** Selects outstanding revision work in the same deterministic order as a wave. */
export const revisionNodes = (graph: Graph): Node[] =>
  // Match scheduling's stable wave order when several nodes request revision.
  graph.nodes
    .filter(
      (node) =>
        node.status === 'needs_revision' &&
        node.outcome !== null &&
        node.outcome.revisionRequest !== null,
    )
    .sort((left, right) => right.index - left.index);

/** Rejects an exact planner-owned no-op while it can still be repaired. */
export const localizedRevisionSchema = (active: Graph, target: Node) =>
  PlannedGraphSchema.superRefine((plan, context) => {
    if (!sameRevisablePlan(active, plan, target.id)) {return;}

    context.addIssue({
      code: 'custom',
      path: ['nodes'],
      message:
        'Localized revision must change at least one planner-owned node field.',
    });
  });

/** Applies planner fields while enforcing localized runtime-state preservation. */
export const applyLocalizedRevision = (
  active: Graph,
  plan: PlannedGraph,
  target: Node,
  retiredIds: ReadonlySet<string>,
): Graph => {
  // A planner response can only revise the node whose decision supplied evidence.
  if (
    target.status !== 'needs_revision' ||
    target.outcome === null ||
    target.outcome.revisionRequest === null
  ) {
    throw new Error('Localized revision target is not awaiting revision.');
  }

  // Reusing a removed ID would make graph history semantically ambiguous.
  const reused = plan.nodes.find(({ id }) => retiredIds.has(id));

  if (reused !== undefined) {
    throw new Error(`Localized revision reused retired node ID ${reused.id}.`);
  }

  // Completed and other started nodes outside the target are immutable history.
  const protectedNodes = active.nodes.filter(
    (node) => node.id !== target.id && node.status !== 'pending',
  );

  // Protected nodes must retain both their identity and their array position.
  for (const node of protectedNodes) {
    const index = active.nodes.indexOf(node);
    const planned = plan.nodes[index];

    if (planned === undefined || !samePlan(node, planned)) {
      throw new Error(`Localized revision changed protected node ${node.id}.`);
    }
  }

  /**
   * Clone protected runtime state exactly. Every other planned node is part of
   * the revisable region and restarts pending with no promoted partial result,
   * routing selection, observation ledger, or consumed revision request.
   */
  const nodes = plan.nodes.map((planned, index) => {
    const protectedNode = protectedNodes.find(({ id }) => id === planned.id);

    if (protectedNode !== undefined) {return cloneNode(protectedNode);}

    return {
      ...planned,
      status: 'pending' as const,
      index,
      candidates: [],
      bundle: null,
      tools: [],
      artifacts: [],
      observations: [],
      outcome: null,
      termination: null,
    };
  });

  // Full graph validation rechecks references, acyclicity, and deliverable rules.
  return GraphSchema.parse({ revision: active.revision + 1, nodes });
};

/** Compares only planner-owned fields when checking a protected node. */
const samePlan = (
  node: Node,
  planned: PlannedGraph['nodes'][number],
): boolean =>
  node.id === planned.id &&
  node.goal === planned.goal &&
  node.deliver === planned.deliver &&
  sameItems(node.doneWhen, planned.doneWhen) &&
  sameItems(node.dependsOn, planned.dependsOn);

const sameRevisablePlan = (
  graph: Graph,
  plan: PlannedGraph,
  targetId: string,
): boolean =>
  graph.nodes.length === plan.nodes.length &&
  graph.nodes.every((node, index) => {
    if (node.id !== targetId && node.status !== 'pending') {return true;}

    const planned = plan.nodes[index];

    return planned !== undefined && samePlan(node, planned);
  });

const sameItems = (
  left: readonly string[],
  right: readonly string[],
): boolean =>
  left.length === right.length &&
  left.every((item, index) => item === right[index]);

/** Copies every nested runtime collection so snapshots remain independent. */
const cloneNode = (node: Node): Node => ({
  ...node,
  doneWhen: [...node.doneWhen],
  dependsOn: [...node.dependsOn],
  candidates: node.candidates.map((candidate) => ({ ...candidate })),
  bundle:
    node.bundle === null
      ? null
      : { ...node.bundle, skills: [...node.bundle.skills] },
  tools: node.tools.map((tool) => ({ ...tool })),
  artifacts: node.artifacts.map((artifact) => ({ ...artifact })),
  outcome:
    node.outcome === null
      ? null
      : {
          ...node.outcome,
          criteria: node.outcome.criteria.map((criterion) => ({
            ...criterion,
            observationIds: [...criterion.observationIds],
          })),
          result:
            node.outcome.result === null
              ? null
              : {
                  ...node.outcome.result,
                  artifacts: node.outcome.result.artifacts.map((artifact) => ({
                    ...artifact,
                  })),
                },
          revisionRequest:
            node.outcome.revisionRequest === null
              ? null
              : { ...node.outcome.revisionRequest },
        },
  observations: node.observations.map((observation) => ({ ...observation })),
  termination:
    node.termination === null
      ? null
      : node.termination.type === 'dependency'
        ? {
            ...node.termination,
            dependencyIds: [...node.termination.dependencyIds],
          }
        : { ...node.termination },
});
