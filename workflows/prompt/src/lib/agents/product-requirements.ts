import { createToolStorage } from 'tools';
import { z } from 'zod';

import type {
  PromptArtifact,
  PromptWorkflowOptions,
} from '../types/prompt.js';
import { withData } from '../utils/artifacts.js';
import { completePass } from '../utils/pass.js';

export const productRequirementsSchema = z
  .object({
    requirements: z.array(z.string()),
  })
  .strict();

export const productRequirementsAgent = {
  name: 'Product requirements pass',
  system: `You are the product requirements pass for a self-contained prompt workflow.

Extract product-facing requirements only when the artifact has no open questions.

Use only information present in the artifact or explicit tool results. Do not infer missing decisions.
Do not write files or ask the user directly. Return data through the supplied structured output schema.`,
  schema: productRequirementsSchema,
} as const;

export const withProductRequirements = async (
  artifact: PromptArtifact,
  options: PromptWorkflowOptions,
): Promise<PromptArtifact> => {
  const response = await completePass(artifact, options, {
    ...productRequirementsAgent,
    tools: createToolStorage([]),
  });

  return withData(artifact, {
    productRequirements: response.requirements,
  });
};
