import { StrictGraphSchema } from '../../schemas/graph.js';
import type { WorkflowHandler } from '../../types/workflow.js';

import * as candidatesPrompts from '../../prompts/candidates.js';
import * as hintsPrompts from '../../prompts/hints.js';
import * as goals from '../../prompts/goals.js';
import * as revisionPrompt from '../../prompts/revision.js';
import { SkillHintExtractionSchema } from '../../schemas/hint.js';

const TOP_K = 10;

/** Creates and logs the execution graph for a workflow run. */
export const graph: WorkflowHandler<'graph'> = async (
  { graphs },
  { input, options },
  { transition, fail },
) => {
  const { provider, models, skills, logger } = options;

  try {
    // Picks the latest graph
    const graph = graphs.at(-1);

    logger.info({
      msg: 'graph',
      graph: graph ?? null,
    });

    // Extract hints for each graph node if available
    const hints = !graph
      ? []
      : await Promise.all(
          graph.nodes.map(async (node) => {
            // Find N Skills
            const matches = await skills.embeddings.search(
              candidatesPrompts.search(input, node),
              TOP_K,
            );

            const promises = matches.map(async ({ data: skill }) => {
              const { structured: result } = await provider.complete({
                messages: [
                  {
                    role: 'system',
                    content: hintsPrompts.system(),
                  },
                  {
                    role: 'user',
                    content: hintsPrompts.user(graph.nodes, node, skill),
                  },
                ],
                model: options.models.default,
                schema: SkillHintExtractionSchema,
              });

              return Boolean(result.hints.length)
                ? { skill, hints: result.hints }
                : null;
            });

            const results = await Promise.all(promises);

            return {
              node: node.id,
              skills: results.filter((result) => !!result),
            };
          }),
        );

    logger.info({
      msg: 'hints',
      hints,
    });

    // Writes the plan for the agent
    const { structured: plan } = await provider.complete({
      messages: [
        {
          role: 'system',
          content: graph ? revisionPrompt.system() : goals.system(),
        },
        {
          role: 'user',
          content: graph
            ? revisionPrompt.user(input, graph, hints)
            : goals.user(input),
        },
      ],
      model: models.default,
      schema: StrictGraphSchema,
    });

    logger.info({
      msg: 'plan',
      plan,
    });

    return transition(graph ? 'prepare' : 'graph', {
      graphs: [...graphs, plan],
    });
  } catch (error) {
    return fail(error);
  }
};
