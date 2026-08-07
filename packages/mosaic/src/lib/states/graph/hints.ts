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
  const required = new Set(skills.required.map(({ name }) => name));
  const catalog = new Map(
    skills.menu
      .filter(({ name }) => !required.has(name))
      .map((skill) => [skill.name, skill]),
  );

  return Promise.all(
    graph.nodes.map(async (node) => {
      const matches = await skills.embeddings.search(
        candidatesPrompts.search(input, node),
        TOP_K,
      );
      const seen = new Set<string>();
      const candidates = matches.flatMap(({ data }) => {
        const skill = catalog.get(data.name);
        if (skill === undefined || seen.has(skill.name)) return [];
        seen.add(skill.name);
        return [skill];
      });

      const promises = candidates.map(async (skill) => {
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
