import { createToolStorage } from 'tools';
import { z } from 'zod';

import type {
  PromptArtifact,
  PromptWorkflowOptions,
} from '../types/prompt.js';
import { withData } from '../utils/artifacts.js';
import { completePass } from '../utils/pass.js';

export const technicalRequirementsSchema = z
  .object({
    requirements: z.array(z.string()),
  })
  .strict();

export const technicalRequirementsAgent = {
  name: 'Technical requirements pass',
  system: `You are the technical requirements pass for a self-contained prompt workflow.

Extract implementation and validation requirements only when the artifact has no open questions.

Use only information present in the artifact or explicit tool results. Do not infer missing decisions.
Do not write files or ask the user directly. Return data through the supplied structured output schema.`,
  schema: technicalRequirementsSchema,
} as const;

export const withTechnicalRequirements = async (
  artifact: PromptArtifact,
  options: PromptWorkflowOptions,
): Promise<PromptArtifact> => {
  const response = await completePass(artifact, options, {
    ...technicalRequirementsAgent,
    tools: createToolStorage([]),
  });

  return withData(artifact, {
    technicalRequirements: response.requirements,
  });
};
