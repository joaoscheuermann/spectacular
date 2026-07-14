import type { Completion, CompletionFor } from './completion.js';
import { createFailure, createPool, type Failure, type Pool } from './pool.js';
import { judgeInput, resultJudgeSystemPrompt } from './prompts.js';
import type { Progress, ProgressEvent } from './progress.js';
import {
  judgeOutputSchema,
  type EvalVerdict,
  type EvolutionConfig,
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
  readonly concurrency?: EvolutionConfig['evolution']['concurrency'];
};

type Runtime = {
  readonly complete: Completion;
  readonly judge: Judge;
  readonly prompt: string;
  readonly options: EvaluationOptions;
  readonly failure: Failure;
  readonly judgments: Pool;
};

type Pair = {
  readonly evaluation: Scenario['evals'][number];
  readonly sampleIndex: number;
  readonly output: string;
};

const report = (
  progress: Progress | undefined,
  failure: Failure,
  event: ProgressEvent,
): void => {
  try {
    progress?.(event);
  } catch (error) {
    throw failure.fail(error);
  }
};

const reportFailure = (
  progress: Progress | undefined,
  failure: Failure,
  event: ProgressEvent,
): void => {
  try {
    progress?.(event);
  } catch (error) {
    failure.fail(error);
  }
};

const protectedCall = async <Value>(
  failure: Failure,
  call: () => Promise<Value>,
): Promise<Value> => {
  try {
    return await call();
  } catch (error) {
    throw failure.fail(error);
  }
};

const values = <Value>(
  settled: readonly PromiseSettledResult<Value>[],
  failure: Failure,
): readonly Value[] => {
  if (failure.failed()) throw failure.reason();
  return settled.map((result) => {
    if (result.status === 'rejected') throw result.reason;
    return result.value;
  });
};

const samples = async (
  scenario: Scenario,
  runtime: Runtime,
): Promise<readonly string[]> => {
  const requests = Array.from({ length: sampleCount }, () =>
    protectedCall(runtime.failure, () =>
      runtime.complete.text(runtime.prompt, scenario.input),
    ),
  );
  return values(await Promise.allSettled(requests), runtime.failure);
};

const pairs = (
  scenario: Scenario,
  outputs: readonly string[],
): readonly Pair[] =>
  scenario.evals.flatMap((evaluation) =>
    outputs.map((output, sampleIndex) => ({
      evaluation,
      sampleIndex,
      output,
    })),
  );

const judgePair = async (
  scenario: Scenario,
  pair: Pair,
  runtime: Runtime,
): Promise<EvalVerdict> => {
  const judged = await runtime.judgments.run(() =>
    runtime.judge.completion.structured(
      resultJudgeSystemPrompt,
      judgeInput({
        scenarioInput: scenario.input,
        assertion: pair.evaluation,
        sampleIndex: pair.sampleIndex,
        modelOutput: pair.output,
      }),
      judgeOutputSchema,
    ),
  );
  return {
    evalId: pair.evaluation.id,
    sampleIndex: pair.sampleIndex,
    ...judged,
  };
};

const judgeScenario = async (
  scenario: Scenario,
  outputs: readonly string[],
  runtime: Runtime,
): Promise<readonly EvalVerdict[]> => {
  const requests = pairs(scenario, outputs).map((pair) =>
    judgePair(scenario, pair, runtime),
  );
  return values(await Promise.allSettled(requests), runtime.failure);
};

const evaluateScenario = async (
  scenario: Scenario,
  runtime: Runtime,
): Promise<ScenarioResult> => {
  const { failure, options } = runtime;
  try {
    report(options.progress, failure, {
      event: 'scenario.start',
      target: options.targetId,
      scenarioId: scenario.id,
      sampleCount,
      evalCount: scenario.evals.length,
    });
    const outputs = await samples(scenario, runtime);
    const verdicts = await judgeScenario(scenario, outputs, runtime);
    const passed = verdicts.filter((verdict) => verdict.passed).length;
    const accuracy = passed / verdicts.length;
    report(options.progress, failure, {
      event: 'scenario.complete',
      target: options.targetId,
      scenarioId: scenario.id,
      accuracy,
      passed,
      total: verdicts.length,
    });
    return { scenario, outputs, verdicts, accuracy };
  } catch (error) {
    failure.fail(error);
    reportFailure(options.progress, failure, {
      event: 'scenario.failed',
      target: options.targetId,
      scenarioId: scenario.id,
    });
    throw failure.reason();
  }
};

const completedEvaluation = (
  results: readonly ScenarioResult[],
): Evaluation => {
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
  return { accuracy, results, failures };
};

/** Evaluates three target samples with one judge call per eval/sample pair. */
export const evaluate = async (
  complete: Completion,
  judge: Judge,
  prompt: string,
  scenarios: readonly Scenario[],
  options: EvaluationOptions,
): Promise<Evaluation> => {
  if (scenarios.length === 0) throw new Error('Evaluation suite is empty.');
  const failure = createFailure();
  report(options.progress, failure, {
    event: 'evaluation.start',
    target: options.targetId,
    scenarioCount: scenarios.length,
  });
  const concurrency = options.concurrency ?? { scenarios: 1, judgments: 1 };
  const judgments = createPool(concurrency.judgments, failure);
  const scenarioPool = createPool(concurrency.scenarios, failure);
  const runtime = { complete, judge, prompt, options, failure, judgments };
  const pending = scenarios.map((scenario) =>
    scenarioPool.run(() => evaluateScenario(scenario, runtime)),
  );
  const settled = await Promise.allSettled(pending);
  await Promise.all([scenarioPool.idle(), judgments.idle()]);
  const result = completedEvaluation(values(settled, failure));
  report(options.progress, failure, {
    event: 'evaluation.complete',
    target: options.targetId,
    accuracy: result.accuracy,
    passed: result.results.reduce(
      (sum, result) =>
        sum + result.verdicts.filter((verdict) => verdict.passed).length,
      0,
    ),
    total: result.results.reduce(
      (sum, scenario) => sum + scenario.verdicts.length,
      0,
    ),
  });
  return result;
};
