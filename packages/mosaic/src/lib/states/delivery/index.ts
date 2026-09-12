import { FinalDeliverySchema } from '../../schemas/delivery.js';
import { GraphSchema } from '../../schemas/graph.js';
import { MosaicResultSchema } from '../../schemas/result.js';
import type { FinalDeliveryPart } from '../../types/delivery.js';
import type { Graph, Node } from '../../types/graph.js';
import type { WorkflowNodeResult } from '../../types/result.js';
import type { WorkflowHandler, WorkflowState } from '../../types/workflow.js';

/**
 * Finalizes terminal node results without invoking models, skills, or tools.
 * Only an all-completed graph receives an assembled delivery.
 */
export const delivery: WorkflowHandler = async (
  state,
  { options, runtime },
  { finish, fail },
) => {
  try {
    const graph = activeGraph(state);
    const ordered = topologicalOrder(graph);

    if (ordered.some(({ status }) => !isTerminal(status))) {
      throw new Error('Cannot finalize an incomplete graph.');
    }

    const nodes = ordered.map(nodeResult);

    const status = nodes.some((node) => node.status === 'failed')
      ? 'failed'
      : nodes.some((node) => node.status === 'blocked')
        ? 'blocked'
        : 'completed';

    if (status !== 'completed') {
      const result = MosaicResultSchema.parse({ status, nodes });

      options.logger.info(
        { status, nodeIds: nodes.map(({ id }) => id) },
        'workflow terminated',
      );

      await runtime?.emit({
        type: 'delivery.created',
        stage: 'delivery',
        partIds: [],
        ...(runtime.capture === 'io' ? { delivery: null } : {}),
      });

      return finish(result);
    }

    const parts = ordered.filter(({ deliver }) => deliver).map(part);

    if (parts.length === 0)
      {throw new Error('Cannot deliver a graph without deliverable nodes.');}

    const delivery = FinalDeliverySchema.parse({
      markdown: parts.map(({ markdown }) => markdown).join('\n\n'),
      parts,
    });
    const result = MosaicResultSchema.parse({ status, delivery, nodes });

    await runtime?.emit({
      type: 'delivery.created',
      stage: 'delivery',
      partIds: delivery.parts.map(({ id }) => id),
      ...(runtime.capture === 'io' ? { delivery } : {}),
    });

    options.logger.info(
      {
        nodeIds: delivery.parts.map(({ id }) => id),
        partCount: delivery.parts.length,
      },
      'delivery assembled',
    );

    return finish(result);
  } catch (error) {
    return fail(error);
  }
};

const isTerminal = (status: Node['status']): boolean =>
  status === 'completed' || status === 'blocked' || status === 'failed';

const activeGraph = (state: WorkflowState): Graph => {
  const graph = state.graphs.at(-1);

  if (graph === undefined) {
    throw new Error('Cannot deliver without an active graph.');
  }

  return GraphSchema.parse(graph);
};

const topologicalOrder = (graph: Graph): Node[] => {
  const positions = new Map(graph.nodes.map((node, index) => [node.id, index]));

  const indegrees = new Map(
    graph.nodes.map((node) => [node.id, node.dependsOn.length]),
  );

  const dependents = new Map(
    graph.nodes.map((node) => [node.id, [] as string[]]),
  );

  for (const node of graph.nodes) {
    for (const dependency of node.dependsOn) {
      dependents.get(dependency)?.push(node.id);
    }
  }

  const ready = graph.nodes
    .filter(({ id }) => indegrees.get(id) === 0)
    .map(({ id }) => id)
    .sort(
      (left, right) => position(positions, left) - position(positions, right),
    );
  const ordered: Node[] = [];

  while (ready.length > 0) {
    const id = ready.shift();

    if (id === undefined) {break;}

    const node = graph.nodes[position(positions, id)];

    if (node === undefined) {throw new Error(`Delivery node is missing: ${id}.`);}

    ordered.push(node);

    for (const dependent of dependents.get(id) ?? []) {
      const next = (indegrees.get(dependent) ?? 0) - 1;

      indegrees.set(dependent, next);

      if (next === 0) {ready.push(dependent);}
    }

    ready.sort(
      (left, right) => position(positions, left) - position(positions, right),
    );
  }

  if (ordered.length !== graph.nodes.length) {
    throw new Error('Cannot deliver a cyclic graph.');
  }

  return ordered;
};

const position = (
  positions: ReadonlyMap<string, number>,
  id: string,
): number => {
  const value = positions.get(id);

  if (value === undefined) {throw new Error(`Delivery node is missing: ${id}.`);}

  return value;
};

const part = (node: Node): FinalDeliveryPart => {
  const [primary, ...artifacts] = node.artifacts;

  if (
    primary === undefined ||
    primary.kind !== 'inline' ||
    primary.mime !== 'text/markdown' ||
    primary.data.length === 0
  ) {
    throw new Error(`Deliverable node ${node.id} has no Markdown result.`);
  }

  return {
    id: node.id,
    goal: node.goal,
    markdown: primary.data,
    artifacts: artifacts.map((artifact) => ({ ...artifact })),
    observations: node.observations.map((observation) => ({ ...observation })),
  };
};

const nodeResult = (node: Node): WorkflowNodeResult => ({
  id: node.id,
  goal: node.goal,
  doneWhen: [...node.doneWhen],
  status: node.status as WorkflowNodeResult['status'],
  candidates: node.candidates.map((candidate) => ({ ...candidate })),
  bundle:
    node.bundle === null
      ? null
      : { ...node.bundle, skills: [...node.bundle.skills] },
  observations: node.observations.map((observation) => ({ ...observation })),
  outcome: node.outcome === null ? null : structuredClone(node.outcome),
  termination:
    node.termination === null ? null : structuredClone(node.termination),
});
