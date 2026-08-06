import type { WorkflowHandler } from '../../types/workflow.js';
// import { prepareNode } from './node.js';

/** Prepares every node in the selected wave sequentially. */
export const bundle: WorkflowHandler<'bundle'> = async (
  { graphs },
  context,
  { transition, fail },
) => {
  try {
    return fail(new Error('Impossible to continue, missing active graph!'))

    // const graph = graphs.at(-1);

    // if (graph === undefined) {
    //   return fail(new Error('Impossible to continue, missing active graph!'));
    // }

    // const nodes = graph.nodes.filter(({ status }) => status === 'ready');

    // for (const node of nodes) {
    //   await prepareNode(node, context);
    // }

    // return transition('schedule', { graphs });
  } catch (error) {
    return fail(error);
  }
};
