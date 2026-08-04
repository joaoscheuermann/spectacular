import type { LlmProvider } from 'llms';
import type { Logger } from 'pino';
import * as revisionPrompt from '../prompts/revision.js';
import { StrictGraphSchema } from '../schemas/graph.js';
import type { Graph } from '../types/graph.js';
import type { SkillExtraction } from '../types/hint.js';

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
