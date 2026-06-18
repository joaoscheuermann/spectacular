import { createToolStorage } from 'tools';
import { z } from 'zod';

import type {
  PromptArtifact,
  PromptWorkflowOptions,
} from '../types/prompt.js';
import { withData } from '../utils/artifacts.js';
import { completePass } from '../utils/pass.js';

export const intentSchema = z
  .object({
    goal: z.string(),
    scope: z.string(),
  })
  .strict();

export const intentAgent = {
  name: 'Intent extraction pass',
  system: `You are the intent extraction pass for a self-contained prompt workflow.

Extract the user's goal and the currently supported scope from the current artifact. If scope is not specified, use an empty string.

Use only information present in the artifact or explicit tool results. Do not infer missing decisions.
Do not write files or ask the user directly. Return data through the supplied structured output schema.`,
  schema: intentSchema,
} as const;

export const withIntent = async (
  artifact: PromptArtifact,
  options: PromptWorkflowOptions,
): Promise<PromptArtifact> => {
  const response = await completePass(artifact, options, {
    ...intentAgent,
    tools: createToolStorage([]),
  });

  return withData(artifact, {
    intent: response,
  });
};
