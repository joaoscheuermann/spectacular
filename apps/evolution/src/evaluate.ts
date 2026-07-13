import type { Completion, CompletionFor } from './completion.js';
import { resultJudgeSystemPrompt } from './prompts.js';
import type { Progress } from './progress.js';
import {
  judgeOutputSchema,
  type EvalVerdict,
  type ModelRef,
  type Scenario,
} from './schema.js';

export const sampleCount = 3;

export type Judge = {
  readonly id: string;
  readonly completion: Completion;
};

export type FailedEval = EvalVerdict & {
  readonly scenarioId: string;
  readonly output: string;
};

export type ScenarioResult = {
  readonly scenario: Scenario;
  readonly outputs: readonly string[];
  readonly verdicts: readonly EvalVerdict[];
  readonly accuracy: number;
};

export type Evaluation = {
  readonly accuracy: number;
  readonly results: readonly ScenarioResult[];
  readonly failures: readonly FailedEval[];
};

/** Composes the configured judge model into a stable identity. */
export const createJudge = (
  model: ModelRef,
  completeFor: CompletionFor,
): Judge => ({
  id: model.provider + ':' + model.model,
  completion: completeFor(model),
});

type EvaluationOptions = {
  readonly targetId: string;
  readonly progress?: Progress;
};

const validateMatrix = (
  scenario: Scenario,
  verdicts: readonly EvalVerdict[],
): readonly EvalVerdict[] => {
  const expected = new Set(
    scenario.evals.flatMap(({ id }) =>
      Array.from({ length: sampleCount }, (_, sampleIndex) =>
        JSON.stringify([id, sampleIndex]),
      ),
    ),
  );
  const actual = verdicts.map(({ evalId, sampleIndex }) =>
    JSON.stringify([evalId, sampleIndex]),
  );
  if (
    actual.length !== expected.size ||
    new Set(actual).size !== actual.length ||
    actual.some((key) => !expected.has(key))
  ) {
    throw new Error(
      `Judge returned an invalid result matrix for scenario "${scenario.id}".`,
    );
  }
  return verdicts;
};

const evaluateScenario = async (
  complete: Completion,
  judge: Judge,
  prompt: string,
  scenario: Scenario,
  options: EvaluationOptions,
): Promise<ScenarioResult> => {
  options.progress?.({
    event: 'scenario.start',
    target: options.targetId,
    scenarioId: scenario.id,
    sampleCount,
    evalCount: scenario.evals.length,
  });
  try {
    const outputs = await Promise.all(
      Array.from({ length: sampleCount }, () =>
        complete.text(prompt, scenario.input),
      ),
    );
    const judged = await judge.completion.structured(
      resultJudgeSystemPrompt,
      JSON.stringify({
        input: scenario.input,
        evals: scenario.evals,
        outputs: outputs.map((output, sampleIndex) => ({
          sampleIndex,
          output,
        })),
      }),
      judgeOutputSchema,
    );
    const verdicts = validateMatrix(scenario, judged.results);
    const passed = verdicts.filter((verdict) => verdict.passed).length;
    const accuracy = passed / verdicts.length;
    options.progress?.({
      event: 'scenario.complete',
      target: options.targetId,
      scenarioId: scenario.id,
      accuracy,
      passed,
      total: verdicts.length,
    });
    return { scenario, outputs, verdicts, accuracy };
  } catch (error) {
    options.progress?.({
      event: 'scenario.failed',
      target: options.targetId,
      scenarioId: scenario.id,
    });
    throw error;
  }
};

/** Evaluates three target samples per scenario with one exact-matrix judge call. */
export const evaluate = async (
  complete: Completion,
  judge: Judge,
  prompt: string,
  scenarios: readonly Scenario[],
  options: EvaluationOptions,
): Promise<Evaluation> => {
  if (scenarios.length === 0) throw new Error('Evaluation suite is empty.');
  options.progress?.({
    event: 'evaluation.start',
    target: options.targetId,
    scenarioCount: scenarios.length,
  });
  const results = await Promise.all(
    scenarios.map((scenario) =>
      evaluateScenario(complete, judge, prompt, scenario, options),
    ),
  );
  const accuracy =
    results.reduce((sum, result) => sum + result.accuracy, 0) / results.length;
  const failures = results.flatMap(({ scenario, outputs, verdicts }) =>
    verdicts.flatMap((verdict) =>
      verdict.passed
        ? []
        : [
            {
              ...verdict,
              scenarioId: scenario.id,
              output: outputs[verdict.sampleIndex] ?? '',
            },
          ],
    ),
  );
  options.progress?.({
    event: 'evaluation.complete',
    target: options.targetId,
    accuracy,
    passed: results.reduce(
      (sum, result) =>
        sum + result.verdicts.filter((verdict) => verdict.passed).length,
      0,
    ),
    total: results.reduce((sum, result) => sum + result.verdicts.length, 0),
  });
  return { accuracy, results, failures };
};
