import { createHash, randomInt } from 'node:crypto';

import { comparisonSystem, comparisonUser } from './prompt.mjs';
import { judgmentChoices, judgmentSchema } from './schemas.mjs';
import { allFulfilled, messages } from './utils.mjs';

export const comparisonFlags = Object.freeze({ sensitiveOutput: true });

export const comparisonProtocolSha256 = () =>
  createHash('sha256')
    .update(comparisonSystem)
    .update('\0')
    .update(
      comparisonUser(
        '__OBJECTIVE__',
        [{ name: '__NAME__', body: '__SKILL_BODY__' }],
        ['__OPTION_A_GOAL__'],
        ['__OPTION_B_GOAL__'],
      ),
    )
    .update('\0')
    .update(
      JSON.stringify({
        schema: {
          choice: judgmentChoices,
          rationale: 'non-whitespace string',
        },
        flags: comparisonFlags,
      }),
    )
    .digest('hex');

export const orientation = (plans, withoutP0Option) => ({
  withoutP0Option,
  optionA: withoutP0Option === 'a' ? plans.withoutP0 : plans.withP0,
  optionB: withoutP0Option === 'b' ? plans.withoutP0 : plans.withP0,
});

export const orientations = (plans) => {
  const firstWithoutP0Option = randomInt(2) === 0 ? 'a' : 'b';
  return [
    orientation(plans, firstWithoutP0Option),
    orientation(plans, firstWithoutP0Option === 'a' ? 'b' : 'a'),
  ];
};

export const outcomeForChoice = (withoutP0Option, choice) => {
  if (choice === 'both' || choice === 'neither') return choice;
  return choice === withoutP0Option ? 'withoutP0' : 'withP0';
};

const samePlan = (left, right) =>
  left.length === right.length &&
  left.every((goal, index) => goal === right[index]);

export const comparisonOutcome = (plans, judgments) => {
  const rawOutcome = judgments.every(
    ({ winner }) => winner === judgments[0].winner,
  )
    ? judgments[0].winner
    : 'inconsistent';
  const plansIdentical = samePlan(plans.withoutP0, plans.withP0);

  return {
    rawOutcome,
    outcome: plansIdentical ? 'both' : rawOutcome,
    plansIdentical,
  };
};

export const summarizeJudgments = (options, responses) =>
  responses.map((response, index) => ({
    orientation: index + 1,
    withoutP0Option: options[index].withoutP0Option,
    choice: response.choice,
    rationale: response.rationale.trim(),
    winner: outcomeForChoice(options[index].withoutP0Option, response.choice),
  }));

export const comparePlans = async ({
  provider,
  action,
  recordUsage,
  recordAttempt,
  config,
  objective,
  skills,
  plans,
  options,
  operation = 'judge',
}) => {
  const responses = await allFulfilled(
    options.map((current, index) =>
      action(
        `Judging orientation ${index + 1}`,
        async () => {
          recordAttempt(operation, config.judgeModel);
          const result = await provider.complete({
            model: config.judgeModel,
            effort: config.judgeEffort,
            messages: messages(
              comparisonSystem,
              comparisonUser(
                objective,
                skills,
                current.optionA,
                current.optionB,
              ),
            ),
            schema: judgmentSchema,
            flags: comparisonFlags,
          });
          recordUsage(operation, config.judgeModel, result.usage);
          return result.structured;
        },
        config.retry,
      ),
    ),
    'One or more judge orientations failed.',
  );
  const judgments = summarizeJudgments(options, responses);

  return { judgments, ...comparisonOutcome(plans, judgments) };
};

const outcomes = ['withoutP0', 'withP0', 'both', 'neither', 'inconsistent'];

const combinations = (n, k) => {
  const smaller = Math.min(k, n - k);
  let result = 1;
  for (let index = 1; index <= smaller; index += 1) {
    result = (result * (n - smaller + index)) / index;
  }
  return result;
};

export const exactBinomialRightTail = (successes, trials) => {
  if (trials === 0) return null;
  let probability = 0;
  for (let count = successes; count <= trials; count += 1) {
    probability += combinations(trials, count) * 0.5 ** trials;
  }
  return Math.min(1, probability);
};

export const aggregateComparisons = (results, judgeModel) => {
  const counts = Object.fromEntries(outcomes.map((outcome) => [outcome, 0]));
  for (const result of results) counts[result.outcome] += 1;

  const decisive = counts.withoutP0 + counts.withP0;
  const withP0PreferenceRate = decisive === 0 ? null : counts.withP0 / decisive;
  const pValue = exactBinomialRightTail(counts.withP0, decisive);
  const alpha = 0.05;
  const decision =
    decisive === 0
      ? 'insufficient_stable_preferences'
      : pValue <= alpha
        ? 'reject_null'
        : 'fail_to_reject_null';

  return {
    primaryJudge: judgeModel,
    counts,
    decisivePreferences: decisive,
    withP0PreferenceRate,
    exactOneSidedBinomialPValue: pValue,
    hypothesis: {
      null: 'P(primary judge prefers withP0) <= 0.5',
      alternative: 'P(primary judge prefers withP0) > 0.5',
      alpha,
      decision,
    },
    identicalPlans: results.filter(({ plansIdentical }) => plansIdentical)
      .length,
  };
};
