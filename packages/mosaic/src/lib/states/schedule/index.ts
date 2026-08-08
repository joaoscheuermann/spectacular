import type { Node } from '../../types/graph.js';
import type { WorkflowHandler } from '../../types/workflow.js';

const WAVE_LIMIT = 5;
const MISSING_READY_NODES = 'Impossible to continue, missing ready nodes!';

/**
 * Projects the lifecycle in MOSAIC 0.2 section 4.10 onto a bounded
 * execution wave, or delegates outstanding structural evidence to revision.
 */
export const schedule: WorkflowHandler = (
  state,
  _context,
  { transition, fail },
) => {
  const { graphs } = state;

  // Planning and revision append snapshots; scheduling always acts on the newest.
  const graph = graphs.at(-1);

  // Every schedulable run must have completed at least the initial planning pass.
  if (graph === undefined) {
    return fail(new Error('Impossible to continue, missing active graph!'));
  }

  /**
   * Revision has precedence over new work so no node executes against a graph
   * whose structure has already been invalidated by runtime evidence.
   */
  if (graph.nodes.some((node) => node.status === 'needs_revision')) {
    // A semantic revision status without its runtime-owned request is invalid state.
    if (
      graph.nodes.some(
        (node) =>
          node.status === 'needs_revision' &&
          (node.outcome === null || node.outcome.revisionRequest === null),
      )
    ) {
      return fail(new Error('Revision node is missing its runtime request.'));
    }

    // The revision state consumes requests deterministically and appends a new graph.
    return transition('revision', state);
  }

  // A terminal non-completed dependency causally blocks each pending descendant.
  propagateDependencyBlocks(graph.nodes);

  // Final assembly is deterministic once every node reaches a terminal status.
  if (graph.nodes.every(({ status }) => isTerminal(status))) {
    return transition('delivery', state);
  }

  /**
   * Select pending goals whose dependencies are complete. Reverse index order is
   * Doric's stable wave order; the fixed cap is a local dispatch policy rather
   * than a concurrency value prescribed by Algorithm 1.
   */
  const wave = graph.nodes
    .filter((node) => nodeIsReady(node, graph.nodes))
    .sort((left, right) => right.index - left.index)
    .slice(0, WAVE_LIMIT);

  // Remaining nonterminal work without an eligible node cannot make DAG progress.
  if (!wave.length) {
    return fail(new Error(MISSING_READY_NODES));
  }

  // Scheduling owns pending -> ready; execution owns every later status change.
  for (const node of wave) {
    node.status = 'ready';
  }

  // Bundle routing and tool-menu composition are performed only for this wave.
  return transition('bundle', state);
};

const isTerminal = (status: Node['status']): boolean =>
  status === 'completed' || status === 'blocked' || status === 'failed';

const propagateDependencyBlocks = (nodes: readonly Node[]): void => {
  let changed = true;

  while (changed) {
    changed = false;
    for (const node of nodes) {
      if (node.status !== 'pending') continue;

      const dependencies = node.dependsOn
        .map((id) => nodes.find((candidate) => candidate.id === id))
        .filter((dependency): dependency is Node => dependency !== undefined);
      if (
        !dependencies.some(
          ({ status }) => status === 'blocked' || status === 'failed',
        )
      ) {
        continue;
      }

      node.status = 'blocked';
      node.outcome = null;
      node.termination = {
        type: 'dependency',
        status: 'blocked',
        dependencyIds: dependencies
          .filter(({ status }) => status !== 'completed')
          .map(({ id }) => id),
      };
      changed = true;
    }
  }
};

/**
 * Applies the readiness rule from section 4.10: a pending node is eligible when
 * it has no dependencies or every referenced dependency has completed.
 */
const nodeIsReady = (node: Node, nodes: readonly Node[]): boolean => {
  // Nodes already admitted to a wave or in a terminal state cannot be rescheduled.
  if (node.status !== 'pending') {
    return false;
  }

  // Root goals are immediately eligible while still pending.
  if (node.dependsOn.length === 0) {
    return true;
  }

  // Graph validation guarantees references exist; completion unlocks the node.
  return nodes
    .filter((candidate) => node.dependsOn.includes(candidate.id))
    .every((dependency) => dependency.status === 'completed');
};
