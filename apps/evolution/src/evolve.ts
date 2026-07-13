import type { Completion, CompletionFor } from './completion.js';
import {
  createJudges,
  evaluate,
  type Evaluation,
  type Judge,
  type ScenarioResult,
} from './evaluate.js';
import { optimizerSystemPrompt } from './prompts.js';
import type { Progress } from './progress.js';
import {
  normalizeInput,
  normalizeProposalScenarios,
  validateScenarioCandidates,
} from './scenarios.js';
import {
  proposalOutputSchema,
  scenarioSchema,
  type EvolutionConfig,
  type ModelRef,
  type Scenario,
  type TargetModel,
} from './schema.js';

export type StopReason = 'target-reached' | 'plateau' | 'max-epochs';

export type HistoryEntry = {
  readonly epoch: number;
  readonly score: number;
  readonly failures: readonly ScenarioResult[];
};

export type TargetResult = {
  readonly target: TargetModel;
  readonly prompt: string;
  readonly evaluation: Evaluation;
  readonly scenarios: readonly Scenario[];
  readonly acceptedScenarios: readonly Scenario[];
  readonly epochsRun: number;
  readonly stopReason: StopReason;
};

type EvolutionOptions = EvolutionConfig['evolution'];

type TargetInputs = {
  readonly target: TargetModel;
  readonly optimizer: ModelRef;
  readonly judges: readonly Judge[];
  readonly prompt: string;
  readonly scenarios: readonly Scenario[];
  readonly options: EvolutionOptions;
  readonly completeFor: CompletionFor;
  readonly progress?: Progress;
};

type TargetState = {
  readonly prompt: string;
  readonly scenarios: readonly Scenario[];
  readonly evaluation: Evaluation;
  readonly history: readonly HistoryEntry[];
  readonly acceptedScenarios: readonly Scenario[];
  readonly epochsRun: number;
  readonly plateauCount: number;
};

type Proposal = {
  readonly prompt: string;
  readonly scenarios: readonly Scenario[];
};

type ProposalInputs = {
  readonly complete: Completion;
  readonly target: TargetModel;
  readonly prompt: string;
  readonly scenarios: readonly Scenario[];
  readonly history: readonly HistoryEntry[];
  readonly progress?: Progress;
};

export const evolutionStopReason = (
  score: number,
  options: EvolutionOptions,
  epochsRun: number,
  plateauCount: number,
): StopReason | undefined => {
  if (score >= options.targetAccuracy) return 'target-reached';
  if (plateauCount >= options.plateauPatience) return 'plateau';
  if (epochsRun >= options.maxEpochs) return 'max-epochs';
  return undefined;
};

const historyEntry = (epoch: number, evaluation: Evaluation): HistoryEntry => ({
  epoch,
  score: evaluation.score,
  failures: evaluation.results.filter(({ passed }) => !passed),
});

const propose = async (inputs: ProposalInputs): Promise<Proposal> => {
  inputs.progress?.({
    event: 'proposal.start',
    target: inputs.target.id,
    scenarioCount: inputs.scenarios.length,
    historyCount: inputs.history.length,
  });
  try {
    const proposal = await inputs.complete.structured(
      optimizerSystemPrompt,
      JSON.stringify({
        target: {
          id: inputs.target.id,
          provider: inputs.target.provider,
          model: inputs.target.model,
        },
        prompt: inputs.prompt,
        scenarios: inputs.scenarios,
        history: inputs.history,
      }),
      proposalOutputSchema,
    );
    inputs.progress?.({
      event: 'proposal.complete',
      target: inputs.target.id,
      scenarioCount: proposal.scenarios.length,
    });
    return {
      prompt: proposal.prompt,
      scenarios: normalizeProposalScenarios(proposal.scenarios),
    };
  } catch (error) {
    inputs.progress?.({
      event: 'proposal.failed',
      target: inputs.target.id,
    });
    throw error;
  }
};

const evaluatePrompt = (
  inputs: TargetInputs,
  complete: Completion,
  prompt: string,
  scenarios: readonly Scenario[],
): Promise<Evaluation> =>
  evaluate(complete, inputs.judges, prompt, scenarios, {
    targetId: inputs.target.id,
    progress: inputs.progress,
  });

const initialState = async (
  inputs: TargetInputs,
  complete: Completion,
): Promise<TargetState> => {
  const evaluation = await evaluatePrompt(
    inputs,
    complete,
    inputs.prompt,
    inputs.scenarios,
  );
  return {
    prompt: inputs.prompt,
    scenarios: [...inputs.scenarios],
    evaluation,
    history: [historyEntry(0, evaluation)],
    acceptedScenarios: [],
    epochsRun: 0,
    plateauCount: 0,
  };
};

const acceptedCandidates = async (
  inputs: TargetInputs,
  state: TargetState,
  proposal: Proposal,
): Promise<readonly Scenario[]> =>
  (
    await validateScenarioCandidates({
      judges: inputs.judges,
      originalPrompt: inputs.prompt,
      incumbents: state.scenarios,
      candidates: proposal.scenarios,
      progress: inputs.progress,
    })
  )
    .filter(({ accepted }) => accepted)
    .map(({ candidate }) => candidate);

const updatedEvaluation = async (
  inputs: TargetInputs,
  complete: Completion,
  prompt: string,
  scenarios: readonly Scenario[],
  fallback: Evaluation,
  acceptedCount: number,
): Promise<Evaluation> =>
  acceptedCount === 0
    ? fallback
    : evaluatePrompt(inputs, complete, prompt, scenarios);

const nextState = async (
  inputs: TargetInputs,
  complete: Completion,
  state: TargetState,
  proposal: Proposal,
  candidate: Evaluation,
  accepted: readonly Scenario[],
): Promise<TargetState> => {
  const promptWon = candidate.score > state.evaluation.score;
  const prompt = promptWon ? proposal.prompt : state.prompt;
  const scenarios = [...state.scenarios, ...accepted];
  const selected = promptWon ? candidate : state.evaluation;
  const evaluation = await updatedEvaluation(
    inputs,
    complete,
    prompt,
    scenarios,
    selected,
    accepted.length,
  );
  const epoch = state.epochsRun + 1;
  return {
    prompt,
    scenarios,
    evaluation,
    history: [...state.history, historyEntry(epoch, evaluation)],
    acceptedScenarios: [...state.acceptedScenarios, ...accepted],
    epochsRun: epoch,
    plateauCount: promptWon ? 0 : state.plateauCount + 1,
  };
};

const reportEpoch = (
  inputs: TargetInputs,
  previous: TargetState,
  next: TargetState,
  acceptedCount: number,
): void => {
  inputs.progress?.({
    event: 'epoch.complete',
    target: inputs.target.id,
    epoch: next.epochsRun,
    score: next.evaluation.score,
    promptWon: next.prompt !== previous.prompt,
    acceptedScenarioCount: acceptedCount,
    plateauCount: next.plateauCount,
  });
};

const runEpoch = async (
  inputs: TargetInputs,
  targetComplete: Completion,
  optimizerComplete: Completion,
  state: TargetState,
): Promise<TargetState> => {
  inputs.progress?.({
    event: 'epoch.start',
    target: inputs.target.id,
    epoch: state.epochsRun + 1,
    score: state.evaluation.score,
    plateauCount: state.plateauCount,
  });
  const proposal = await propose({
    complete: optimizerComplete,
    target: inputs.target,
    prompt: state.prompt,
    scenarios: state.scenarios,
    history: state.history,
    progress: inputs.progress,
  });
  const candidate = await evaluatePrompt(
    inputs,
    targetComplete,
    proposal.prompt,
    state.scenarios,
  );
  const accepted = await acceptedCandidates(inputs, state, proposal);
  const next = await nextState(
    inputs,
    targetComplete,
    state,
    proposal,
    candidate,
    accepted,
  );
  reportEpoch(inputs, state, next, accepted.length);
  return next;
};

/** Evolves one model without sharing prompt state, scenarios, or history. */
export const evolveTarget = async (
  inputs: TargetInputs,
): Promise<TargetResult> => {
  if (inputs.scenarios.length === 0) {
    throw new Error(
      'Evolution requires a non-empty baseline; initialize scenarios before evolving models.',
    );
  }
  const targetComplete = inputs.completeFor(inputs.target);
  const optimizerComplete = inputs.completeFor(inputs.optimizer);
  let state = await initialState(inputs, targetComplete);
  let stopReason = evolutionStopReason(
    state.evaluation.score,
    inputs.options,
    state.epochsRun,
    state.plateauCount,
  );
  while (stopReason === undefined) {
    state = await runEpoch(inputs, targetComplete, optimizerComplete, state);
    stopReason = evolutionStopReason(
      state.evaluation.score,
      inputs.options,
      state.epochsRun,
      state.plateauCount,
    );
  }

  return {
    target: inputs.target,
    prompt: state.prompt,
    evaluation: state.evaluation,
    scenarios: state.scenarios,
    acceptedScenarios: state.acceptedScenarios,
    epochsRun: state.epochsRun,
    stopReason,
  };
};

/** Starts every target from the same original prompt and scenario snapshot. */
export const evolveModels = async (
  config: EvolutionConfig,
  prompt: string,
  scenarios: readonly Scenario[],
  completeFor: CompletionFor,
  progress?: Progress,
): Promise<readonly TargetResult[]> => {
  if (scenarios.length === 0) {
    throw new Error(
      'Evolution requires a non-empty baseline; initialize scenarios before evolving models.',
    );
  }
  const judges = createJudges(config.judges, completeFor);
  const results: TargetResult[] = [];
  for (const target of config.models) {
    results.push(
      await evolveTarget({
        target,
        optimizer: config.optimizer,
        judges,
        prompt,
        scenarios: [...scenarios],
        options: config.evolution,
        completeFor,
        progress,
      }),
    );
  }
  return results;
};

const sameScenario = (left: Scenario, right: Scenario): boolean =>
  JSON.stringify(scenarioSchema.parse(left)) ===
  JSON.stringify(scenarioSchema.parse(right));

/** Merges independently accepted scenarios and fails on cross-target collisions. */
export const mergeScenarios = (
  initial: readonly Scenario[],
  targets: readonly TargetResult[],
): readonly Scenario[] => {
  const byId = new Map(initial.map((scenario) => [scenario.id, scenario]));
  const byInput = new Map(
    initial.map((scenario) => [normalizeInput(scenario.input), scenario.id]),
  );

  for (const scenario of targets.flatMap(
    ({ acceptedScenarios }) => acceptedScenarios,
  )) {
    const existing = byId.get(scenario.id);
    if (existing !== undefined) {
      if (sameScenario(existing, scenario)) continue;
      throw new Error('Scenario id collision: ' + scenario.id);
    }
    const inputOwner = byInput.get(normalizeInput(scenario.input));
    if (inputOwner !== undefined) {
      throw new Error(
        'Scenario input collision between "' +
          inputOwner +
          '" and "' +
          scenario.id +
          '".',
      );
    }
    byId.set(scenario.id, scenario);
    byInput.set(normalizeInput(scenario.input), scenario.id);
  }
  return [...byId.values()].sort((a, b) => a.id.localeCompare(b.id));
};
