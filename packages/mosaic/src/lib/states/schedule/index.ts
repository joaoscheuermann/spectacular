import type { Graph, Node } from '../../types/graph.js';
import type { WorkflowHandler } from '../../types/workflow.js';

const WAVE_LIMIT = 5;
const MISSING_READY_NODES = 'Impossible to continue, missing ready nodes!';

export type ScheduleResult =
  | { readonly status: 'finished' }
  | { readonly status: 'failed'; readonly error: Error }
  | { readonly status: 'ready'; readonly nodes: readonly Node[] };

/** Mutates the active graph to mark its next wave as ready. */
export const schedule: WorkflowHandler<'schedule'> = (
  { graphs },
  _context,
  { transition, finish, fail },
) => {
  const graph = graphs.at(-1);

  if (graph === undefined) {
    return fail(new Error('Impossible to continue, missing active graph!'));
  }

  const result = selectWave(graph);

  if (result.status === 'finished') {
    return finish();
  }

  if (result.status === 'failed') {
    return fail(result.error);
  }

  return transition('prepare', { graphs });
};

/** Selects and marks the next ready wave on the active graph. */
export const selectWave = (graph: Graph): ScheduleResult => {
  if (graph.nodes.every((node) => node.status === 'completed')) {
    return { status: 'finished' };
  }

  const ready = graph.nodes
    .filter((node) => isReady(node, graph.nodes))
    .sort((left, right) => right.index - left.index)
    .slice(0, WAVE_LIMIT);

  if (ready.length === 0) {
    return { status: 'failed', error: new Error(MISSING_READY_NODES) };
  }

  for (const node of ready) {
    node.status = 'ready';
  }

  return {
    status: 'ready',
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
