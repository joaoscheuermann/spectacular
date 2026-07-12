import { readFile, readdir } from 'node:fs/promises';
import { join } from 'node:path';

import type { Judge, JudgeResult } from './evaluate.js';
import { scenarioJudgeSystemPrompt } from './prompts.js';
import type { Progress } from './progress.js';
import {
  judgmentSchema,
  scenarioSchema,
  type ProposalOutput,
  type Scenario,
} from './schema.js';

export type ScenarioValidationReason =
  | 'duplicate-id'
  | 'duplicate-input'
  | 'insufficient-judges'
  | 'judge-failed'
  | 'ambiguous'
  | 'judge-disagreement'
  | 'rejected';

export type ScenarioValidation = {
  readonly candidate: Scenario;
  readonly accepted: boolean;
  readonly reason?: ScenarioValidationReason;
  readonly judgments: readonly JudgeResult[];
};

type ValidationInputs = {
  readonly judges: readonly Judge[];
  readonly originalPrompt: string;
  readonly incumbents: readonly Scenario[];
  readonly candidates: readonly Scenario[];
  readonly progress?: Progress;
};

export const normalizeInput = (value: string): string =>
  value.trim().replace(/\s+/gu, ' ').toLocaleLowerCase();

const assertUnique = (scenarios: readonly Scenario[]): void => {
  const ids = scenarios.map(({ id }) => id);
  if (new Set(ids).size !== ids.length) {
    throw new Error('Scenario ids must be unique.');
  }
  const inputs = scenarios.map(({ input }) => normalizeInput(input));
  if (new Set(inputs).size !== inputs.length) {
    throw new Error('Scenario inputs must be unique.');
  }
};

/** Loads scenario JSON files in stable lexical order. */
export const loadScenarios = async (
  directory: string,
): Promise<readonly Scenario[]> => {
  let names: readonly string[];
  try {
    names = (await readdir(directory))
      .filter((name) => name.endsWith('.json'))
      .sort();
  } catch (error) {
    if (error instanceof Error && 'code' in error && error.code === 'ENOENT') {
      return [];
    }
    throw error;
  }
  const scenarios = await Promise.all(
    names.map(async (name) =>
      scenarioSchema.parse(
        JSON.parse(await readFile(join(directory, name), 'utf8')),
      ),
    ),
  );
  assertUnique(scenarios);
  return scenarios;
};

export const normalizeProposalScenarios = (
  scenarios: ProposalOutput['scenarios'],
): readonly Scenario[] =>
  scenarios.map(({ rationale, ...scenario }) =>
    scenarioSchema.parse({
      ...scenario,
      ...(rationale === null ? {} : { rationale }),
    }),
  );

const duplicateReason = (
  candidate: Scenario,
  ids: Set<string>,
  inputs: Set<string>,
): ScenarioValidationReason | undefined => {
  if (ids.has(candidate.id)) return 'duplicate-id';
  if (inputs.has(normalizeInput(candidate.input))) return 'duplicate-input';
  return undefined;
};

const judgeCandidate = async (
  judges: readonly Judge[],
  originalPrompt: string,
  candidate: Scenario,
): Promise<readonly PromiseSettledResult<JudgeResult>[]> =>
  Promise.allSettled(
    judges.map(async ({ id, completion }) => ({
      ...(await completion.structured(
        scenarioJudgeSystemPrompt,
        JSON.stringify({
          originalPrompt,
          input: candidate.input,
          expected: candidate.expected,
        }),
        judgmentSchema,
      )),
      judge: id,
    })),
  );

const judgmentReason = (
  settled: readonly PromiseSettledResult<JudgeResult>[],
  judgments: readonly JudgeResult[],
): ScenarioValidationReason | undefined => {
  if (settled.some((result) => result.status === 'rejected')) {
    return 'judge-failed';
  }
  if (judgments.some(({ ambiguous }) => ambiguous)) return 'ambiguous';
  if (new Set(judgments.map(({ passed }) => passed)).size > 1) {
    return 'judge-disagreement';
  }
  return judgments.every(({ passed }) => passed) ? undefined : 'rejected';
};

const validateCandidate = async (
  judges: readonly Judge[],
  originalPrompt: string,
  candidate: Scenario,
  duplicate: ScenarioValidationReason | undefined,
): Promise<ScenarioValidation> => {
  if (duplicate !== undefined) {
    return {
      candidate,
      accepted: false,
      reason: duplicate,
      judgments: [],
    };
  }
  if (judges.length < 2) {
    return {
      candidate,
      accepted: false,
      reason: 'insufficient-judges',
      judgments: [],
    };
  }
  const settled = await judgeCandidate(judges, originalPrompt, candidate);
  const judgments = settled.flatMap((result) =>
    result.status === 'fulfilled' ? [result.value] : [],
  );
  const reason = judgmentReason(settled, judgments);
  return {
    candidate,
    accepted: reason === undefined,
    ...(reason === undefined ? {} : { reason }),
    judgments,
  };
};

/** Screens additions for duplicates and unanimous, unambiguous judge approval. */
export const validateScenarioCandidates = async (
  inputs: ValidationInputs,
): Promise<readonly ScenarioValidation[]> => {
  const ids = new Set(inputs.incumbents.map(({ id }) => id));
  const normalizedInputs = new Set(
    inputs.incumbents.map(({ input }) => normalizeInput(input)),
  );
  const results: ScenarioValidation[] = [];
  inputs.progress?.({
    event: 'scenario-validation.start',
    candidateCount: inputs.candidates.length,
    judgeCount: inputs.judges.length,
  });

  for (const candidate of inputs.candidates) {
    const duplicate = duplicateReason(candidate, ids, normalizedInputs);
    ids.add(candidate.id);
    normalizedInputs.add(normalizeInput(candidate.input));
    const result = await validateCandidate(
      inputs.judges,
      inputs.originalPrompt,
      candidate,
      duplicate,
    );
    results.push(result);
    inputs.progress?.({
      event: result.accepted
        ? 'scenario-validation.accepted'
        : 'scenario-validation.rejected',
      scenarioId: candidate.id,
      ...(result.reason === undefined ? {} : { reason: result.reason }),
    });
  }

  inputs.progress?.({
    event: 'scenario-validation.complete',
    candidateCount: inputs.candidates.length,
    acceptedCount: results.filter(({ accepted }) => accepted).length,
  });
  return results;
};
