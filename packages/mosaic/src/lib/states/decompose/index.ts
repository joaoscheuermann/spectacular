import { decompose as createGraph } from '../../decomposition/index.js';
import type { WorkflowHandler } from '../../types/workflow.js';

/** Creates and logs the execution graph for a workflow run. */
export const decompose: WorkflowHandler<'decompose'> = async (
  _artifacts,
  { context, transition, fail },
) => {
  try {
    const { input, options } = context;

    const graph = await createGraph(input, {
      logger: options.logger,
      provider: options.provider,
      vectors: options.skills.embeddings,
      model: options.models.default,
    });

    options.logger.info({ msg: 'generated the execution plan' });
    console.log(JSON.stringify(graph));

    return transition('schedule', { graph });
  } catch (error) {
    return fail(error);
  }
};
