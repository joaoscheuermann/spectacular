import type { WorkflowHandler } from '../../types/workflow.js';
import { prepareNode } from './node.js';

/** Prepares every node in the selected wave sequentially. */
export const prepare: WorkflowHandler<'prepare'> = async (
  { graph, nodes },
  { context, transition, fail },
) => {
  try {
    for (const node of nodes) {
      await prepareNode(node, context);
    }

    return transition('schedule', { graph });
  } catch (error) {
    return fail(error);
  }
};
