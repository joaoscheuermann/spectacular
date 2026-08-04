import type { LlmProvider } from 'llms';
import * as goalsPrompt from '../prompts/goals.js';
import { StrictGraphSchema } from '../schemas/graph.js';

interface DecomposeContext {
  provider: LlmProvider;
}

export default async function goals(
  model: string,
  prompt: string,
  { provider }: DecomposeContext,
) {
  const { structured } = await provider.complete({
    messages: [
      {
        role: 'system',
        content: goalsPrompt.system(),
      },
      {
        role: 'user',
        content: goalsPrompt.user(prompt),
      },
    ],
    model,
    schema: StrictGraphSchema,
  });

  return structured;
}
