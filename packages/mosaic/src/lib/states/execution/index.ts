import type { StateMachineHandler } from 'state-machine';

import type { WorkflowContext, WorkflowState } from '../../types/workflow.js';

/** Placeholder execution state that completes the current workflow run. */
export const execution: StateMachineHandler<WorkflowContext, WorkflowState> = (
  { graphs },
  { options },
  { finish, fail },
) => {
  const graph = graphs.at(-1)

  if (graph === undefined)
    return fail(new Error('Impossible to continue, missing active graph!'));

  const { logger } = options

  /**
   * Gets only nodes that are ready!
   */
  const nodes = graph.nodes.filter(node => node.status === 'ready');

  if (!nodes.length)
    return fail(new Error('Impossible to continue, missing ready nodes! Why are you here?'));

  logger.info({ graph })

  return finish()
};
