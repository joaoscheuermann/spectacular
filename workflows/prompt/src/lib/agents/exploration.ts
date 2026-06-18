import { z } from 'zod';

import { createSafeExplorationTools } from '../tools/exploration.js';
import type {
  PromptArtifact,
  PromptWorkflowOptions,
} from '../types/prompt.js';
import { withData } from '../utils/artifacts.js';
import { completePass } from '../utils/pass.js';

export const explorationSchema = z
  .object({
    summary: z.string(),
    facts: z.array(
      z
        .object({
          fact: z.string(),
          evidencePaths: z.array(z.string()),
        })
        .strict(),
    ),
  })
  .strict();

export const explorationAgent = {
  name: 'Exploration pass',
  system: `You are the exploration pass for a self-contained prompt workflow.

Explore the workspace only when repository facts are useful for understanding the request. Use only the read-only tools available to this pass.

Use only information present in the artifact or explicit tool results. Do not infer missing decisions.
Do not write files or ask the user directly. Return data through the supplied structured output schema.`,
  schema: explorationSchema,
} as const;

export const withExploration = async (
  artifact: PromptArtifact,
  options: PromptWorkflowOptions,
): Promise<PromptArtifact> => {
  const response = await completePass(artifact, options, {
    ...explorationAgent,
    tools: createSafeExplorationTools(options.workspaceRoot),
  });

  return withData(artifact, {
    exploration: response,
  });
};
