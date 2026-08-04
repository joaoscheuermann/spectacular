import { LlmProvider } from 'llms';
import { Logger } from 'pino';
import { VectorDatabase } from 'victor';
import * as hintsPrompt from '../../prompts/hints/index.js';
import { SkillHintExtractionSchema } from '../../schemas/hint/index.js';
import { SkillExtraction } from '../../types/hint.js';
import type { Skill } from 'bundle';
import { Graph } from '../../types/graph.js';
import { GoalSkills } from '../candidates/index.js';

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

    logger.info({
      msg: 'extracting hints for goal',
      goal: node.goal,
    });

    return bundle.skills.map(async (skill) => {
      logger.info({
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
