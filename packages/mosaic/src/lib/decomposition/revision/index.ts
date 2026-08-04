import { LlmProvider } from 'llms';
import { Logger } from 'pino';
import * as revisionPrompt from '../../prompts/revision/index.js';
import { StrictGraphSchema } from '../../schemas/graph/index.js';
import { Graph } from '../../types/graph.js';
import { SkillExtraction } from '../../types/hint.js';

interface DecomposeContext {
  provider: LlmProvider;
  logger: Logger;
}

export default async function revision(
  model: string,
  prompt: string,
  plan: Graph,
  hints: Set<SkillExtraction>,
  { provider }: DecomposeContext,
) {
  const { structured } = await provider.complete({
    messages: [
      {
        role: 'system',
        content: revisionPrompt.system(),
      },
      {
        role: 'user',
        content: revisionPrompt.user(prompt, plan, hints),
      },
    ],
    model,
    schema: StrictGraphSchema,
  });

  return structured;
}
