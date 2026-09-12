import * as z from 'zod';

import { pairLabel, summarize, summarizeJudgment } from './comparison.mjs';
import { comparisonSystem, comparisonUser } from './prompts.mjs';
import { providerFailure, retryProvider } from './retry.mjs';

const judgeSchema = z
  .object({
    choice: z.enum(['a', 'b', 'both', 'neither']),
    rationale: z.string().trim().min(1),
  })
  .strict();

const judgePlans = async ({
  deps,
  expectedSkills,
  current,
  round,
  pair,
  judgeModel,
  orientation,
  options,
}) => {
  const result = await deps.action(
    `${current.name} ${round}/${deps.config.rounds}: judging ${pairLabel(pair)} with ${judgeModel} orientation ${orientation}/2`,
    () =>
      retryProvider(
        () =>
          deps.provider.complete({
            model: judgeModel,
            effort: deps.config.judgeEffort,
            messages: [
              {
                role: 'system',
                content: comparisonSystem(
                  current.objective,
                  expectedSkills.map(({ body }) => body),
                ),
              },
              {
                role: 'user',
                content: comparisonUser(options),
              },
            ],
            schema: judgeSchema,
            flags: { sensitiveOutput: true },
          }),
        {
          operation: 'judge_completion',
          model: judgeModel,
          caseName: current.name,
          round,
          pair: pair.id,
          orientation,
          onFailure: deps.onProviderFailure,
        },
      ),
  );

  return result.structured;
};

const failedOrientation = (options, index, error) => {
  const failure = providerFailure(error);

  if (failure === null) {throw error;}

  return {
    orientation: index + 1,
    leftOption: options[index].leftOption,
    ...failure,
  };
};

export const compareWithJudge = async ({
  deps,
  current,
  round,
  pair,
  expectedSkills,
  options,
  judgeModel,
}) => {
  const responses = await Promise.allSettled(
    options.map((currentOptions, index) =>
      judgePlans({
        deps,
        expectedSkills,
        current,
        round,
        pair,
        judgeModel,
        orientation: index + 1,
        options: currentOptions,
      }),
    ),
  );

  const judgments = responses.flatMap((response, index) =>
    response.status === 'fulfilled'
      ? [summarizeJudgment(pair, options, response.value, index)]
      : [],
  );

  const failures = responses.flatMap((response, index) =>
    response.status === 'rejected'
      ? [failedOrientation(options, index, response.reason)]
      : [],
  );

  if (failures.length === 0) {
    return {
      judgeModel,
      status: 'completed',
      ...summarize(
        pair,
        options,
        responses.map(({ value }) => value),
      ),
    };
  }

  return {
    judgeModel,
    status: judgments.length === 0 ? 'failed' : 'partial',
    ...pair,
    judgments,
    failures,
    winner: null,
  };
};
