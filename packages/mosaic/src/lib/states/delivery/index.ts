import { GraphSchema } from '../../schemas/graph.js';
import { FinalDeliverySchema } from '../../schemas/delivery.js';
import type { Graph, Node } from '../../types/graph.js';
import type { FinalDeliveryPart } from '../../types/delivery.js';
import type { WorkflowHandler, WorkflowState } from '../../types/workflow.js';

/**
 * Assembles completed terminal node results without invoking models, skills, or
 * tools. Its output is the workflow's only successful finish value.
 */
export const delivery: WorkflowHandler = (
  state,
  { options },
  { finish, fail },
) => {
  try {
    const graph = activeGraph(state);
    const ordered = topologicalOrder(graph);

    if (ordered.some(({ status }) => status !== 'completed')) {
      throw new Error('Cannot deliver an incomplete graph.');
    }

    const parts = ordered.filter(({ deliver }) => deliver).map(part);
    if (parts.length === 0) {
      throw new Error('Cannot deliver a graph without deliverable nodes.');
    }

    const result = FinalDeliverySchema.parse({
      markdown: parts.map(({ markdown }) => markdown).join('\n\n'),
      parts,
    });

    options.logger.info(
      {
        nodeIds: result.parts.map(({ id }) => id),
        partCount: result.parts.length,
      },
      'delivery assembled',
    );
    return finish(result);
  } catch (error) {
    return fail(error);
  }
};

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
    if (id === undefined) break;
    const node = graph.nodes[position(positions, id)];
    if (node === undefined) throw new Error(`Delivery node is missing: ${id}.`);
    ordered.push(node);

    for (const dependent of dependents.get(id) ?? []) {
      const next = (indegrees.get(dependent) ?? 0) - 1;
      indegrees.set(dependent, next);
      if (next === 0) ready.push(dependent);
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
  if (value === undefined) throw new Error(`Delivery node is missing: ${id}.`);
  return value;
};

const part = (node: Node): FinalDeliveryPart => {
  const [primary, ...artifacts] = node.artifacts;
  if (
    primary === undefined ||
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
