import type { LlmProvider } from 'llms';
import type { Logger } from 'pino';
import type { VectorDatabase } from 'victor';
import * as hintsPrompt from '../prompts/hints.js';
import { SkillHintExtractionSchema } from '../schemas/hint.js';
import type { SkillExtraction } from '../types/hint.js';
import type { Skill } from 'bundle';
import type { Graph } from '../types/graph.js';
import type { GoalSkills } from './candidates.js';

interface HintsContext {
  logger: Logger;
  provider: LlmProvider;
  vectors: VectorDatabase<Skill>;
}

export default async function hints(
  model: string,
  graph: Graph,
  skills: Array<GoalSkills>,
  { provider, logger }: HintsContext,
): Promise<Set<SkillExtraction>> {
  const requests = skills.flatMap((bundle) => {
    const node = graph.nodes.find((node) => node.id === bundle.goal);

    if (!node) throw new Error('Goal not found!');

    logger.debug({
      msg: 'extracting hints for goal',
      goal: node.goal,
    });

    return bundle.skills.map(async (skill) => {
      logger.debug({
        msg: 'extracting hints from skill',
        goal: node.goal,
        skill: skill.name,
        description: skill.description,
      });

      const { structured } = await provider.complete({
        messages: [
          {
            role: 'system',
            content: hintsPrompt.system(),
          },
          {
            role: 'user',
            content: hintsPrompt.user(graph.nodes, node, skill),
          },
        ],
        model,
        schema: SkillHintExtractionSchema,
      });

      if (!structured.hints.length) return;

      return {
        goal: node.goal,
        skill,
        hints: structured.hints,
      };
    });
  });

  const extractions = (await Promise.all(requests)).filter(
    (extraction): extraction is SkillExtraction => extraction !== undefined,
  );

  return new Set(extractions);
}
