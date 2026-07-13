import type { Completion } from './completion.js';
import type { Judge, JudgeResult } from './evaluate.js';
import { scenarioInitializerSystemPrompt } from './prompts.js';
import type { Progress } from './progress.js';
import {
  normalizeProposalScenarios,
  validateScenarioCandidates,
  type ScenarioValidation,
  type ScenarioValidationReason,
} from './scenarios.js';
import { scenarioInitializationOutputSchema, type Scenario } from './schema.js';

type Inputs = {
  readonly optimizer: Completion;
  readonly judges: readonly Judge[];
  readonly originalPrompt: string;
  readonly maxAttempts: number;
  readonly progress?: Progress;
};

type Rejection = {
  readonly scenarioId: string;
  readonly reason: ScenarioValidationReason;
  readonly judgments: readonly JudgeResult[];
};

type Feedback = {
  readonly attemptedCandidates: readonly Scenario[];
  readonly rejections: readonly Rejection[];
};

const rejectedFeedback = (
  validations: readonly ScenarioValidation[],
): readonly Rejection[] =>
  validations
    .filter(
      (
        validation,
      ): validation is ScenarioValidation & {
        readonly accepted: false;
        readonly reason: ScenarioValidationReason;
      } => !validation.accepted && validation.reason !== undefined,
    )
    .map(({ candidate, reason, judgments }) => ({
      scenarioId: candidate.id,
      reason,
      judgments: judgments.map(({ judge, passed, ambiguous, rationale }) => ({
        judge,
        passed,
        ambiguous,
        rationale,
      })),
    }));

const judgeFailure = (validations: readonly ScenarioValidation[]): boolean =>
  validations.some(
    ({ reason }) =>
      reason === 'judge-failed' || reason === 'insufficient-judges',
  );

const proposalInput = (originalPrompt: string, feedback: Feedback): string =>
  JSON.stringify({ originalPrompt, feedback });

/** Establishes the first unanimously approved scenario batch before target evolution. */
export const initializeScenarios = async (
  inputs: Inputs,
): Promise<readonly Scenario[]> => {
  inputs.progress?.({
    event: 'scenario-initialization.start',
    maxAttemptCount: inputs.maxAttempts,
  });
  let attemptedCandidates: readonly Scenario[] = [];
  let rejections: readonly Rejection[] = [];
  let attemptCount = 0;

  try {
    for (let attempt = 1; attempt <= inputs.maxAttempts; attempt += 1) {
      attemptCount = attempt;
      inputs.progress?.({
        event: 'scenario-initialization.attempt.start',
        attempt,
      });
      const proposal = await inputs.optimizer.structured(
        scenarioInitializerSystemPrompt,
        proposalInput(inputs.originalPrompt, {
          attemptedCandidates,
          rejections,
        }),
        scenarioInitializationOutputSchema,
      );
      const candidates = normalizeProposalScenarios(proposal.scenarios);
      const validations = await validateScenarioCandidates({
        judges: inputs.judges,
        originalPrompt: inputs.originalPrompt,
        incumbents: attemptedCandidates,
        candidates,
        progress: inputs.progress,
      });
      const accepted = validations
        .filter(({ accepted }) => accepted)
        .map(({ candidate }) => candidate);
      inputs.progress?.({
        event: 'scenario-initialization.attempt.complete',
        attempt,
        candidateCount: candidates.length,
        acceptedCount: accepted.length,
        rejectedCount: candidates.length - accepted.length,
      });

      if (judgeFailure(validations)) {
        throw new Error(
          'Scenario initialization failed because configured judges could not validate a candidate.',
        );
      }
      if (accepted.length > 0) {
        inputs.progress?.({
          event: 'scenario-initialization.complete',
          attemptCount: attempt,
          scenarioCount: accepted.length,
        });
        return accepted;
      }

      attemptedCandidates = [...attemptedCandidates, ...candidates];
      rejections = [...rejections, ...rejectedFeedback(validations)];
    }

    throw new Error(
      'Scenario initialization failed after ' +
        inputs.maxAttempts +
        ' attempts because no candidate received unanimous, unambiguous judge approval.',
    );
  } catch (error) {
    inputs.progress?.({
      event: 'scenario-initialization.failed',
      attemptCount,
    });
    throw error;
  }
};
