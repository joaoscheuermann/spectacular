import { createToolStorage } from 'tools';
import { z } from 'zod';

import type {
  PromptArtifact,
  PromptOpenQuestion,
  PromptOpenQuestionOptions,
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
          options: z.array(z.string().min(1)).min(3),
        })
        .strict(),
    ),
  })
  .strict();

type OpenQuestionResponse = z.infer<typeof openQuestionsSchema>['questions'][number];

const toOpenQuestion = (question: OpenQuestionResponse): PromptOpenQuestion => ({
  ...question,
  options: toOpenQuestionOptions(question.options),
});

const toOpenQuestionOptions = (
  options: readonly string[],
): PromptOpenQuestionOptions => {
  const [first, second, third, ...rest] = options;

  if (first === undefined || second === undefined || third === undefined) {
    throw new Error(
      'Open question options must include at least 3 options before storing on the prompt artifact.',
    );
  }

  return [first, second, third, ...rest];
};

export const openQuestionsAgent = (maxQuestions: number) =>
  ({
    name: 'Open question pass',
    system: `You are the open question pass for a self-contained prompt workflow.

Find ambiguity between the user request, exploration knowledge, and current artifact state. Include only questions that block reliable product or technical requirement extraction. Return no more than ${maxQuestions} questions.

For every returned question, include at least 3 concrete solution options the user can select. Each option must be an actionable solution choice, not a meta-suggestion like "define a prioritized subset". Make recommendation exactly match one of the options, preferably the first option.

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
  const openQuestions = response.questions
    .slice(0, maxQuestions)
    .map(toOpenQuestion);

  return {
    artifact: withData(artifact, {
      openQuestions,
    }),
    hasBlockingQuestions: response.questions.length > 0,
  };
};
