import { createToolStorage } from 'tools';
import { z } from 'zod';

import type {
  PromptArtifact,
  PromptWorkflowOptions,
} from '../types/prompt.js';
import { withData } from '../utils/artifacts.js';
import { completePass } from '../utils/pass.js';

export const openQuestionsSchema = z
  .object({
    questions: z.array(
      z
        .object({
          question: z.string(),
          impact: z.string().optional(),
          recommendation: z.string(),
        })
        .strict(),
    ),
  })
  .strict();

export const openQuestionsAgent = (maxQuestions: number) =>
  ({
    name: 'Open question pass',
    system: `You are the open question pass for a self-contained prompt workflow.

Find ambiguity between the user request, exploration knowledge, and current artifact state. Include only questions that block reliable product or technical requirement extraction. Return no more than ${maxQuestions} questions.

Use only information present in the artifact or explicit tool results. Do not infer missing decisions.
Do not write files or ask the user directly. Return data through the supplied structured output schema.`,
    schema: openQuestionsSchema,
  }) as const;

export type OpenQuestionsPassResult = {
  readonly artifact: PromptArtifact;
  readonly hasBlockingQuestions: boolean;
};

export const withOpenQuestions = async (
  artifact: PromptArtifact,
  options: PromptWorkflowOptions,
): Promise<OpenQuestionsPassResult> => {
  const maxQuestions = Math.max(options.maxQuestions ?? 3, 0);
  const response = await completePass(artifact, options, {
    ...openQuestionsAgent(maxQuestions),
    tools: createToolStorage([]),
  });

  return {
    artifact: withData(artifact, {
      openQuestions: response.questions.slice(0, maxQuestions),
    }),
    hasBlockingQuestions: response.questions.length > 0,
  };
};
