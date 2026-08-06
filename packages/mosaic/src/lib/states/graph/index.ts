import type { StateMachineHandler } from 'state-machine';

import type { WorkflowContext, WorkflowState } from '../../types/workflow.js';

import { hints } from './hints.js';

import * as goals from '../../prompts/goals.js';
import * as revision from '../../prompts/revision.js';

import { StrictGraphSchema } from '../../schemas/graph.js';

/** Creates the execution graph for a workflow run. */
export const graph: StateMachineHandler<
  WorkflowContext,
  WorkflowState
> = async ({ graphs }, { input, options }, { transition, fail }) => {
  const { logger, provider, models } = options;

  try {
    const graph = graphs.at(-1);

    logger.info({
      graph: graph ?? null
    }, 'generating graph');

    const { structured: p } = await provider.complete({
      messages: [
        {
          role: 'system',
          content: graph ? revision.system() : goals.system(),
        },
        {
          role: 'user',
          content: graph
            ? revision.user(input, graph, await hints(input, graph, options))
            : goals.user(input),
        },
      ],
      model: models.default,
      schema: StrictGraphSchema,
    });

    logger.debug(
      {
        revised: graph !== undefined,
      },
      'graph plan completed',
    );

    // Goes to the defined state
    return transition(
      graph
        ? 'schedule'
        : 'graph',
      {
        graphs: [...graphs, p],
      }
    );
  } catch (error) {
    return fail(error);
  }
};
