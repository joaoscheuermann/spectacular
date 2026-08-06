import type { MosaicOptions } from '../../types/mosaic-options.js';
import type { Graph } from '../../types/graph.js';

import * as candidatesPrompts from '../../prompts/candidates.js';
import * as hintsPrompts from '../../prompts/hints.js';
import { SkillHintExtractionSchema } from '../../schemas/hint.js';
import type { Skill } from 'bundle';
import type { SkillHint } from '../../types/hint.js';

const TOP_K = 10;

export type HintGroup = {
  readonly node: string;
  readonly skills: Array<{
    readonly skill: Skill;
    readonly hints: SkillHint[];
  }>;
};

export async function hints(
  input: string,
  graph: Graph,
  options: MosaicOptions,
): Promise<HintGroup[]> {
  const { provider, skills, models } = options;

  return Promise.all(
    graph.nodes.map(async (node) => {
      const matches = await skills.embeddings.search(
        candidatesPrompts.search(input, node),
        TOP_K,
      );

      const promises = matches.map(async ({ data: skill }) => {
        const { structured } = await provider.complete({
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
          model: models.default,
          schema: SkillHintExtractionSchema,
        });

        return structured.hints.length > 0
          ? { skill, hints: structured.hints }
          : null;
      });

      const results = await Promise.all(promises);

      return {
        node: node.id,
        skills: results.filter((result) => result !== null),
      };
    }),
  );
}
