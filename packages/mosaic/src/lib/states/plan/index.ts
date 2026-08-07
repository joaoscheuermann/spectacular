import type { StateMachineHandler } from 'state-machine';

import * as goalsPrompt from '../../prompts/goals.js';
import * as revisionPrompt from '../../prompts/revision.js';
import { materializeGraph, PlannedGraphSchema } from '../../schemas/graph.js';
import type { WorkflowContext, WorkflowState } from '../../types/workflow.js';
import { hints } from './hints.js';

/**
 * Implements the two planning passes from MOSAIC 0.2, sections 4.3-4.4
 * (pp. 12-13): catalog-independent P0 followed by exactly one body-aware P1.
 */
export const plan: StateMachineHandler<WorkflowContext, WorkflowState> = async (
  state,
  { input, options },
  { transition, fail },
) => {
  try {
    // P0 and P1 exhaust the initial planning lifecycle; later changes use revision.
    if (state.graphs.length >= 2) {
      return fail(new Error('Graph generation supports only P0 and P1.'));
    }

    // Graph snapshots form a LIFO history: no active graph means P0, otherwise P1.
    const active = state.graphs.at(-1);
    const phase = active === undefined ? 'p0' : 'p1';
    options.logger.info(
      {
        phase,
        revision: state.graphs.length,
        nodeCount: active?.nodes.length ?? 0,
      },
      'generating graph',
    );

    /**
     * P0 sees only the request and must describe result-oriented goals. P1 sees
     * P0 plus bounded catalog evidence, allowing one informed decomposition pass.
     */
    const { structured } = await options.provider.complete({
      messages: [
        {
          role: 'system',
          content:
            active === undefined
              ? goalsPrompt.system()
              : revisionPrompt.system(),
        },
        {
          role: 'user',
          content:
            active === undefined
              ? goalsPrompt.user(input)
              : revisionPrompt.user(
                  input,
                  active,
                  await hints(input, active, options),
                ),
        },
      ],
      model: options.models.default,
      schema: PlannedGraphSchema,
    });

    // Validate the model-owned planning fields before adding runtime-owned state.
    const planned = PlannedGraphSchema.parse(structured);

    // Materialization assigns pending status, stable indices, and empty ledgers.
    const next = materializeGraph(planned);

    options.logger.debug(
      { phase, revision: state.graphs.length, nodeCount: next.nodes.length },
      'graph plan completed',
    );

    /**
     * P0 loops once so hints can be derived from its goals. P1 advances to the
     * scheduler, where routing is performed again against the final objectives.
     */
    return transition(active === undefined ? 'plan' : 'schedule', {
      ...state,
      graphs: [...state.graphs, next],
    });
  } catch (error) {
    // Provider and graph-contract failures terminate planning without a snapshot.
    return fail(error);
  }
};
