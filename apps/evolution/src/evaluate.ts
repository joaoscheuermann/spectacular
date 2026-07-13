import type { Completion, CompletionFor } from './completion.js';
import { resultJudgeSystemPrompt } from './prompts.js';
import type { Progress } from './progress.js';
import {
  judgmentSchema,
  type Judgment,
  type ModelRef,
  type Scenario,
} from './schema.js';

export type Judge = {
  readonly id: string;
  readonly completion: Completion;
};

export type JudgeResult = Judgment & {
  readonly judge: string;
};

export type ScenarioResult = {
  readonly scenario: Scenario;
  readonly output: string;
  readonly passed: boolean;
  readonly ambiguous: boolean;
  readonly judgments: readonly JudgeResult[];
};

export type Evaluation = {
  readonly score: number;
  readonly passed: number;
  readonly total: number;
  readonly results: readonly ScenarioResult[];
};

/** Composes configured model references into stable judge identities. */
export const createJudges = (
  models: readonly ModelRef[],
  completeFor: CompletionFor,
): readonly Judge[] =>
  models.map((model) => ({
    id: model.provider + ':' + model.model,
    completion: completeFor(model),
  }));

type EvaluationOptions = {
  readonly targetId: string;
  readonly progress?: Progress;
};

const judgeOutput = async (
  judges: readonly Judge[],
  scenario: Scenario,
  output: string,
): Promise<readonly JudgeResult[]> =>
  Promise.all(
    judges.map(async ({ id, completion }) => ({
      ...(await completion.structured(
        resultJudgeSystemPrompt,
        JSON.stringify({
          input: scenario.input,
          expected: scenario.expected,
          output,
        }),
        judgmentSchema,
      )),
      judge: id,
    })),
  );

const evaluateScenario = async (
  complete: Completion,
  judges: readonly Judge[],
  prompt: string,
  scenario: Scenario,
  options: EvaluationOptions,
): Promise<ScenarioResult> => {
  options.progress?.({
    event: 'scenario.start',
    target: options.targetId,
    scenarioId: scenario.id,
  });

  try {
    const output = await complete.text(prompt, scenario.input);
    const judgments = await judgeOutput(judges, scenario, output);
    const ambiguous = judgments.some((judgment) => judgment.ambiguous);
    const passed = !ambiguous && judgments.every((judgment) => judgment.passed);
    options.progress?.({
      event: 'scenario.complete',
      target: options.targetId,
      scenarioId: scenario.id,
      judgeCount: judgments.length,
      passed,
      ambiguous,
    });
    return { scenario, output, judgments, passed, ambiguous };
  } catch (error) {
    options.progress?.({
      event: 'scenario.failed',
      target: options.targetId,
      scenarioId: scenario.id,
    });
    throw error;
  }
};

/** Evaluates arbitrary text behavior without exposing bodies to progress logs. */
export const evaluate = async (
  complete: Completion,
  judges: readonly Judge[],
  prompt: string,
  scenarios: readonly Scenario[],
  options: EvaluationOptions,
): Promise<Evaluation> => {
  options.progress?.({
    event: 'evaluation.start',
    target: options.targetId,
    scenarioCount: scenarios.length,
  });
  const results = await Promise.all(
    scenarios.map((scenario) =>
      evaluateScenario(complete, judges, prompt, scenario, options),
    ),
  );
  const passed = results.filter((result) => result.passed).length;
  const evaluation = {
    score: results.length === 0 ? 0 : passed / results.length,
    passed,
    total: results.length,
    results,
  };
  options.progress?.({
    event: 'evaluation.complete',
    target: options.targetId,
    score: evaluation.score,
    passed,
    total: results.length,
  });
  return evaluation;
};
