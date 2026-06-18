import { createToolStorage } from 'tools';
import { z } from 'zod';

import type {
  PromptArtifact,
  PromptWorkflowOptions,
} from '../types/prompt.js';
import { withData } from '../utils/artifacts.js';
import { completePass } from '../utils/pass.js';

export const requestUnderstandingSchema = z
  .object({
    summary: z.string(),
  })
  .strict();

export const requestUnderstandingAgent = {
  name: 'Request understanding pass',
  system: `You are the request understanding pass for a self-contained prompt workflow.

Summarize what the user asked for based on the current artifact and exploration knowledge.

Use only information present in the artifact or explicit tool results. Do not infer missing decisions.
Do not write files or ask the user directly. Return data through the supplied structured output schema.`,
  schema: requestUnderstandingSchema,
} as const;

export const withRequestUnderstanding = async (
  artifact: PromptArtifact,
  options: PromptWorkflowOptions,
): Promise<PromptArtifact> => {
  const response = await completePass(artifact, options, {
    ...requestUnderstandingAgent,
    tools: createToolStorage([]),
  });

  return withData(artifact, {
    requestUnderstanding: response.summary,
  });
};
