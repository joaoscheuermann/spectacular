import type { StateMachineHandler } from 'state-machine';

import type { Node } from '../../types/graph.js';
import type { WorkflowContext, WorkflowState } from '../../types/workflow.js';

const WAVE_LIMIT = 5;
const MISSING_READY_NODES = 'Impossible to continue, missing ready nodes!';

  /**
   * Mutates the active graph to mark its next wave as ready.
   * Search for all nodes that are currently pending but can become ready
   * @param param0
   * @param _context
   * @param param2
   * @returns
   */
export const schedule: StateMachineHandler<WorkflowContext, WorkflowState> = (
  { graphs },
  _context,
  { transition, finish, fail },
) => {
  const graph = graphs.at(-1);

  if (graph === undefined) {
    return fail(new Error('Impossible to continue, missing active graph!'));
  }

  // All nodes are done, so we mark the graph as finished and finishes the run
  if (graph.nodes.every((node) => node.status === 'completed')) {
    return finish();
  }

  /**
   * Pick wave for the current turn, we evaluate if a node is ready and then limit the size of the wave
   */
  const wave = graph.nodes
    .filter((node) => nodeIsReady(node, graph.nodes))
    .sort((left, right) => right.index - left.index)
    .slice(0, WAVE_LIMIT);

  if (!wave.length) {
    return fail(new Error(MISSING_READY_NODES));
  }

  for (const node of wave) {
    node.status = 'ready';
  }

  return transition('bundle', { graphs });
};

/**
 * Filters the nodes to check if it can be set as ready
 * A node is ready when all it's dependencies are completed or it doesnt have any dependencies
 * @param node
 * @param nodes
 * @returns
 */
const nodeIsReady = (node: Node, nodes: readonly Node[]): boolean => {
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
