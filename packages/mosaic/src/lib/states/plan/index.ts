import * as goalsPrompt from '../../prompts/goals.js';
import * as revisionPrompt from '../../prompts/revision.js';
import {
  GraphHistorySchema,
  materializeGraph,
  PlannedGraphSchema,
} from '../../schemas/graph.js';
import { completeStructured } from '../../structured.js';
import type { WorkflowHandler } from '../../types/workflow.js';
import { hints } from './hints.js';

/**
 * Implements the two planning passes from MOSAIC 0.2, sections 4.4-4.5:
 * catalog-independent P0 followed by exactly one body-aware P1.
 */
export const plan: WorkflowHandler = async (
  state,
  { input, options },
  { transition, fail },
) => {
  try {
    GraphHistorySchema.parse(state.graphs);

    // P0 and P1 exhaust the initial planning lifecycle; later changes use revision.
    const active = state.graphs.at(-1);
    if (active !== undefined && active.revision >= 1) {
      return fail(new Error('Graph generation supports only P0 and P1.'));
    }

    // No active graph means P0; the runtime alone assigns the next revision.
    const phase = active === undefined ? 'p0' : 'p1';
    const revision = active === undefined ? 0 : active.revision + 1;
    options.logger.info(
      {
        phase,
        revision,
        nodeCount: active?.nodes.length ?? 0,
      },
      'generating graph',
    );

    /**
     * P0 sees only the request and must describe result-oriented goals. P1 sees
     * P0 plus bounded catalog evidence, allowing one informed decomposition pass.
     */
    const planned = await completeStructured({
      provider: options.provider,
      model: options.models.default,
      system:
        active === undefined ? goalsPrompt.system() : revisionPrompt.system(),
      input:
        active === undefined
          ? goalsPrompt.user(input)
          : revisionPrompt.user(
              input,
              active,
              await hints(input, active, options),
            ),
      schema: PlannedGraphSchema,
    });

    // Materialization assigns pending status, stable indices, and empty ledgers.
    const next = materializeGraph(planned, revision);

    options.logger.debug(
      { phase, revision: next.revision, nodeCount: next.nodes.length },
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
