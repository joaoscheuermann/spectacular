import type { Graph, Node } from '../../types/graph.js';
import type { WorkflowHandler } from '../../types/workflow.js';

const WAVE_LIMIT = 5;
const MISSING_READY_NODES = 'Impossible to continue, missing ready nodes!';

export type ScheduleResult =
  | { readonly status: 'finished' }
  | { readonly status: 'failed'; readonly error: Error }
  | {
      readonly status: 'ready';
      readonly graph: Graph;
      readonly nodes: readonly Node[];
    };

/** Advances the workflow with the next immutable ready wave. */
export const schedule: WorkflowHandler<'schedule'> = (
  { graph },
  { transition, finish, fail },
) => {
  const result = selectWave(graph);

  if (result.status === 'finished') {
    return finish();
  }

  if (result.status === 'failed') {
    return fail(result.error);
  }

  return transition('prepare', {
    graph: result.graph,
    nodes: result.nodes,
  });
};

/** Selects the next ready wave without mutating the provider-produced graph. */
export const selectWave = (graph: Graph): ScheduleResult => {
  if (graph.nodes.every((node) => node.status === 'completed')) {
    return { status: 'finished' };
  }

  const ready = graph.nodes
    .filter((node) => isReady(node, graph.nodes))
    .sort((left, right) => right.index - left.index)
    .slice(0, WAVE_LIMIT)
    .map((node) => ({ ...node, status: 'ready' as const }));

  if (ready.length === 0) {
    return { status: 'failed', error: new Error(MISSING_READY_NODES) };
  }

  const selected = new Map(ready.map((node) => [node.id, node]));

  return {
    status: 'ready',
    graph: {
      ...graph,
      nodes: graph.nodes.map((node) => selected.get(node.id) ?? node),
    },
    nodes: ready,
  };
};

const isReady = (node: Node, nodes: readonly Node[]): boolean => {
  if (node.status !== 'pending') {
    return false;
  }

  if (node.dependsOn.length === 0) {
    return true;
  }

  return nodes
    .filter((candidate) => node.dependsOn.includes(candidate.id))
    .every((dependency) => dependency.status === 'completed');
};
