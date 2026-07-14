import type { Completion, CompletionFor } from './completion.js';
import {
  createJudge,
  evaluate,
  type Evaluation,
  type Judge,
} from './evaluate.js';
import {
  historyFingerprint,
  optimizerHistory,
  type HistoryDisposition,
  type HistoryRecord,
  type OptimizerMode,
  type TerminalStatus,
} from './history.js';
import {
  compressionInput,
  compressionSystemPrompt,
  optimizerInput,
  optimizerSystemPrompt,
} from './prompts.js';
import type { Progress } from './progress.js';
import {
  proposalOutputSchema,
  type EvolutionConfig,
  type ModelRef,
  type ProposalOutput,
  type Scenario,
  type TargetModel,
} from './schema.js';

export type StopReason = TerminalStatus;

export type TargetResult = {
  readonly target: TargetModel;
  readonly prompt: string;
  readonly trainingEvaluation: Evaluation;
  readonly validationEvaluation?: Evaluation;
  readonly trainingAccuracy: number;
  readonly validationAccuracy?: number;
  readonly approved: boolean;
  readonly refactored: boolean;
  readonly epochsRun: number;
  readonly stopReason: StopReason;
  readonly history: readonly HistoryRecord[];
};

type TargetInputs = {
  readonly target: TargetModel;
  readonly optimizer: ModelRef;
  readonly judge: Judge;
  readonly judgeModel: ModelRef;
  readonly originalPrompt: string;
  readonly training: readonly Scenario[];
  readonly validation: readonly Scenario[];
  readonly globalEvals: EvolutionConfig['evals'];
  readonly options: EvolutionConfig['evolution'];
  readonly completeFor: CompletionFor;
  readonly optimizerIsCodex: boolean;
  readonly previousHistory: readonly HistoryRecord[];
  readonly progress?: Progress;
};

type State = {
  readonly prompt: string;
  readonly evaluation: Evaluation;
  readonly attempts: readonly HistoryRecord[];
  readonly epochsRun: number;
  readonly normalMisses: number;
};

type Runtime = {
  readonly history?: Readonly<Record<string, readonly HistoryRecord[]>>;
  readonly historyErrors?: Readonly<Record<string, Error>>;
  readonly warn?: (message: string) => void;
  readonly onResult?: (result: TargetResult) => void | Promise<void>;
};

const normalized = (value: string): string =>
  value.trim().replace(/\s+/gu, ' ').toLocaleLowerCase();

const optimizerModel = (
  model: ModelRef,
  temperature: number,
  isCodex: boolean,
): ModelRef => {
  const withoutTemperature: ModelRef = { ...model };
  delete withoutTemperature.temperature;
  return isCodex ? withoutTemperature : { ...withoutTemperature, temperature };
};

const evaluatePrompt = (
  inputs: TargetInputs,
  target: Completion,
  prompt: string,
  scenarios: readonly Scenario[],
): Promise<Evaluation> =>
  evaluate(target, inputs.judge, prompt, scenarios, {
    targetId: inputs.target.id,
    progress: inputs.progress,
    concurrency: inputs.options.concurrency,
  });

const propose = async (
  inputs: TargetInputs,
  state: State,
  mode: OptimizerMode,
): Promise<ProposalOutput> => {
  const temperature = mode === 'plateau-escape' ? 0.8 : 0.2;
  const complete = inputs.completeFor(
    optimizerModel(inputs.optimizer, temperature, inputs.optimizerIsCodex),
  );
  const durableHistory = [...inputs.previousHistory, ...state.attempts].slice(
    -inputs.options.history.limit,
  );
  const history = optimizerHistory(durableHistory);
  inputs.progress?.({
    event: 'proposal.start',
    target: inputs.target.id,
    epoch: state.epochsRun + 1,
    optimizerMode: mode,
    historyCount: history.length,
  });
  const proposal = await complete.structured(
    optimizerSystemPrompt,
    optimizerInput({
      prompt: state.prompt,
      trainingScenarios: inputs.training,
      currentFailures: state.evaluation.failures,
      history,
      prohibitedPrompts: history.map(({ attemptedPrompt }) => attemptedPrompt),
      prohibitedStrategies: history
        .filter(
          ({ disposition }) =>
            disposition !== 'improved' &&
            disposition !== 'compression-accepted',
        )
        .map(({ strategy }) => strategy),
    }),
    proposalOutputSchema,
  );
  inputs.progress?.({
    event: 'proposal.complete',
    target: inputs.target.id,
    epoch: state.epochsRun + 1,
    optimizerMode: mode,
  });
  return proposal;
};

const rejectedDisposition = (
  proposal: ProposalOutput,
  state: State,
  previous: readonly HistoryRecord[],
): HistoryDisposition | undefined => {
  const history = [...previous, ...state.attempts];
  const prompts = [
    state.prompt,
    ...history.map(({ attemptedPrompt }) => attemptedPrompt),
  ];
  if (
    prompts.some((prompt) => normalized(prompt) === normalized(proposal.prompt))
  ) {
    return 'duplicate-prompt';
  }
  const failedStrategies = history.filter(
    ({ disposition }) =>
      disposition !== 'improved' && disposition !== 'compression-accepted',
  );
  return failedStrategies.some(
    ({ strategy }) => normalized(strategy) === normalized(proposal.strategy),
  )
    ? 'rejected-strategy'
    : undefined;
};

const record = (
  fingerprint: string,
  proposal: ProposalOutput,
  mode: OptimizerMode,
  evaluation: Evaluation,
  disposition: HistoryDisposition,
): HistoryRecord => ({
  fingerprint,
  attemptedPrompt: proposal.prompt,
  strategy: proposal.strategy,
  optimizerMode: mode,
  trainingAccuracy: evaluation.accuracy,
  failedEvals: evaluation.failures,
  disposition,
  terminalStatus: null,
});

const runEpoch = async (
  inputs: TargetInputs,
  target: Completion,
  state: State,
  mode: 'normal' | 'plateau-escape',
  fingerprint: string,
): Promise<{ readonly state: State; readonly improved: boolean }> => {
  inputs.progress?.({
    event: 'epoch.start',
    target: inputs.target.id,
    epoch: state.epochsRun + 1,
    trainingAccuracy: state.evaluation.accuracy,
    optimizerMode: mode,
  });
  const proposal = await propose(inputs, state, mode);
  const rejected = rejectedDisposition(proposal, state, inputs.previousHistory);
  const candidate =
    rejected === undefined
      ? await evaluatePrompt(inputs, target, proposal.prompt, inputs.training)
      : state.evaluation;
  const improved =
    rejected === undefined && candidate.accuracy > state.evaluation.accuracy;
  const disposition = rejected ?? (improved ? 'improved' : 'not-improved');
  const next: State = {
    prompt: improved ? proposal.prompt : state.prompt,
    evaluation: improved ? candidate : state.evaluation,
    attempts: [
      ...state.attempts,
      record(fingerprint, proposal, mode, candidate, disposition),
    ],
    epochsRun: state.epochsRun + 1,
    normalMisses:
      improved || mode === 'plateau-escape' ? 0 : state.normalMisses + 1,
  };
  inputs.progress?.({
    event: 'epoch.complete',
    target: inputs.target.id,
    epoch: next.epochsRun,
    trainingAccuracy: next.evaluation.accuracy,
    improved,
    disposition,
    optimizerMode: mode,
  });
  return { state: next, improved };
};

const compression = async (
  inputs: TargetInputs,
  target: Completion,
  state: State,
  fingerprint: string,
): Promise<{ readonly state: State; readonly refactored: boolean }> => {
  const complete = inputs.completeFor(
    optimizerModel(inputs.optimizer, 0.2, inputs.optimizerIsCodex),
  );
  inputs.progress?.({ event: 'compression.start', target: inputs.target.id });
  const proposal = await complete.structured(
    compressionSystemPrompt,
    compressionInput({
      prompt: state.prompt,
      trainingScenarios: inputs.training,
    }),
    proposalOutputSchema,
  );
  const originalLength = state.prompt.trim().length;
  const candidateLength = proposal.prompt.trim().length;
  const ratio = originalLength === 0 ? 1 : candidateLength / originalLength;
  const validLength = ratio >= 0.7 && ratio <= 0.8;
  const candidate = validLength
    ? await evaluatePrompt(inputs, target, proposal.prompt, inputs.training)
    : state.evaluation;
  const accepted = validLength && candidate.accuracy >= inputs.options.accuracy;
  const selected = accepted ? candidate : state.evaluation;
  const next = {
    ...state,
    prompt: accepted ? proposal.prompt : state.prompt,
    evaluation: selected,
    attempts: [
      ...state.attempts,
      record(
        fingerprint,
        proposal,
        'compression',
        candidate,
        accepted ? 'compression-accepted' : 'compression-rejected',
      ),
    ],
  };
  inputs.progress?.({
    event: 'compression.complete',
    target: inputs.target.id,
    accepted,
  });
  return { state: next, refactored: accepted };
};

const terminalHistory = (
  records: readonly HistoryRecord[],
  status: StopReason,
): readonly HistoryRecord[] =>
  records.map((entry, index) =>
    index === records.length - 1 ? { ...entry, terminalStatus: status } : entry,
  );

/** Evolves, optionally compresses, then validates one target in isolation. */
export const evolveTarget = async (
  inputs: TargetInputs,
): Promise<TargetResult> => {
  const target = inputs.completeFor(inputs.target);
  const fingerprint = historyFingerprint({
    originalPrompt: inputs.originalPrompt,
    training: inputs.training,
    globalEvals: inputs.globalEvals,
    accuracy: inputs.options.accuracy,
    target: inputs.target,
    judge: inputs.judgeModel,
  });
  let state: State = {
    prompt: inputs.originalPrompt,
    evaluation: await evaluatePrompt(
      inputs,
      target,
      inputs.originalPrompt,
      inputs.training,
    ),
    attempts: [],
    epochsRun: 0,
    normalMisses: 0,
  };
  let trainingStop: 'plateau' | 'max-epochs' | undefined;
  while (state.evaluation.accuracy < inputs.options.accuracy) {
    if (state.epochsRun >= inputs.options.epochs) {
      trainingStop = 'max-epochs';
      break;
    }
    const mode =
      state.normalMisses >= inputs.options.patience.epochs
        ? 'plateau-escape'
        : 'normal';
    const epoch = await runEpoch(inputs, target, state, mode, fingerprint);
    state = epoch.state;
    if (mode === 'plateau-escape' && !epoch.improved) {
      trainingStop = 'plateau';
      break;
    }
  }

  if (trainingStop !== undefined) {
    return {
      target: inputs.target,
      prompt: state.prompt,
      trainingEvaluation: state.evaluation,
      trainingAccuracy: state.evaluation.accuracy,
      approved: false,
      refactored: false,
      epochsRun: state.epochsRun,
      stopReason: trainingStop,
      history: terminalHistory(state.attempts, trainingStop),
    };
  }

  const compressed = await compression(inputs, target, state, fingerprint);
  state = compressed.state;
  const validationEvaluation = await evaluatePrompt(
    inputs,
    target,
    state.prompt,
    inputs.validation,
  );
  const approved = validationEvaluation.accuracy >= inputs.options.accuracy;
  const stopReason = approved ? 'approved' : 'validation-failed';
  return {
    target: inputs.target,
    prompt: state.prompt,
    trainingEvaluation: state.evaluation,
    validationEvaluation,
    trainingAccuracy: state.evaluation.accuracy,
    validationAccuracy: validationEvaluation.accuracy,
    approved,
    refactored: compressed.refactored,
    epochsRun: state.epochsRun,
    stopReason,
    history: terminalHistory(state.attempts, stopReason),
  };
};

/** Starts every target from the same prompt and keeps validation target-local. */
export const evolveModels = async (
  config: EvolutionConfig,
  prompt: string,
  scenarios: readonly Scenario[],
  completeFor: CompletionFor,
  progress?: Progress,
  runtime: Runtime = {},
): Promise<readonly TargetResult[]> => {
  const training = scenarios.filter(({ split }) => split === 'train');
  const validation = scenarios.filter(({ split }) => split === 'validation');
  if (training.length === 0 || validation.length === 0) {
    throw new Error(
      'Evolution requires non-empty train and validation splits.',
    );
  }
  const judge = createJudge(config.judge, completeFor);
  const optimizerProvider = config.providers.find(
    ({ id }) => id === config.optimizer.provider,
  );
  const optimizerIsCodex = optimizerProvider?.type === 'codex';
  if (optimizerIsCodex) {
    (runtime.warn ?? console.error)(
      'Evolution optimizer temperature is omitted for the Codex provider.',
    );
  }
  const results: TargetResult[] = [];
  const errors: Error[] = [];
  for (const target of config.models) {
    try {
      const historyError = runtime.historyErrors?.[target.id];
      if (historyError !== undefined) throw historyError;
      const result = await evolveTarget({
        target,
        optimizer: config.optimizer,
        judge,
        judgeModel: config.judge,
        originalPrompt: prompt,
        training,
        validation,
        globalEvals: config.evals,
        options: config.evolution,
        completeFor,
        optimizerIsCodex,
        previousHistory: runtime.history?.[target.id] ?? [],
        progress,
      });
      results.push(result);
      await runtime.onResult?.(result);
    } catch (error) {
      errors.push(
        new Error(`Evolution failed for target "${target.id}".`, {
          cause: error,
        }),
      );
    }
  }
  if (errors.length > 0) {
    throw new AggregateError(errors, 'One or more evolution targets failed.');
  }
  return results;
};
