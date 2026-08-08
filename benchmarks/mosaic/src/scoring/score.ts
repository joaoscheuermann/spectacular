import type { ScoreRow } from '../schemas/index.js';
import { ScoreRowV1 } from '../schemas/index.js';
import { artifactHash } from '../core/hash.js';
import { ndcgAtK, recallAtK, scoreObservations } from './metrics.js';
import type { ScoreInput } from './types.js';

const mean = (values: readonly number[]): number | null =>
  values.length === 0
    ? null
    : values.reduce((total, value) => total + value, 0) / values.length;

const exactOrder = (
  left: readonly string[],
  right: readonly string[],
): boolean =>
  left.length === right.length &&
  left.every((value, index) => value === right[index]);

const successful = (input: ScoreInput): boolean => {
  const required = input.evidence.assertions.filter(
    (assertion) => assertion.required !== false,
  );
  return (
    input.record.status === 'succeeded' &&
    required.length > 0 &&
    required.every((assertion) => assertion.passed)
  );
};

const retrieval = (input: ScoreInput): ScoreRow['retrieval'] => {
  const items = input.evidence.retrievals ?? [];
  if (items.length === 0) return null;
  const k = items[0]?.k;
  if (k === undefined || items.some((item) => item.k !== k)) {
    throw new TypeError(
      'one score row cannot aggregate different retrieval K values',
    );
  }
  const defaultRelevance = Object.fromEntries([
    ...input.case.gold.relevantSkills.map((id) => [id, 1] as const),
    ...input.case.gold.requiredSkills.map((id) => [id, 2] as const),
  ]);
  return {
    recallAtK: mean(
      items.map((item) =>
        recallAtK(item.ranked, item.relevance ?? defaultRelevance, k),
      ),
    ) as number,
    ndcg: mean(
      items.map((item) =>
        ndcgAtK(item.ranked, item.relevance ?? defaultRelevance, k),
      ),
    ) as number,
    k,
  };
};

const failureCode = (input: ScoreInput, success: boolean): string | null => {
  if (success) return null;
  if (input.evidence.failureCode) return input.evidence.failureCode;
  if (input.record.infrastructureFailure) {
    return input.record.infrastructureFailure.code;
  }
  if (input.record.status === 'failed') return 'execution_failed';
  if (input.record.status === 'infrastructure') return 'infrastructure_failure';
  return 'deterministic_assertion_failed';
};

const sameModel = (
  left: ScoreInput['record']['run']['model'],
  right: ScoreInput['record']['run']['model'],
): boolean =>
  left.provider === right.provider &&
  left.model === right.model &&
  left.effort === right.effort;

const primaryEligible = (input: ScoreInput): boolean => {
  const { eligibility, record } = input;
  if (eligibility.family === 'exploratory') return false;
  const expectedPhase =
    eligibility.family === 'replication' ? 'replication' : 'confirmatory';
  const expectedModel =
    eligibility.family === 'replication'
      ? eligibility.freeze.replication.candidate
      : eligibility.freeze.primaryModel;
  const expectedBudget =
    eligibility.family === 'sensitivity'
      ? eligibility.freeze.modelCallBudgetP95
      : undefined;
  return (
    record.run.phase === expectedPhase &&
    (record.run.conditionId === eligibility.freeze.selectedBaseline ||
      record.run.conditionId === 'M1') &&
    sameModel(record.run.model, expectedModel) &&
    record.run.freezeHash === eligibility.freeze.manifestHash &&
    record.run.modelCallBudget === expectedBudget
  );
};

const validateEligibilityPolicy = (input: ScoreInput): void => {
  const { eligibility } = input;
  if (eligibility.family !== 'sensitivity') return;
  if (
    input.record.run.modelCallBudget !== eligibility.freeze.modelCallBudgetP95
  ) {
    throw new TypeError(
      'sensitivity execution does not match the frozen model-call budget',
    );
  }
};

/** Scores one immutable execution and validates the canonical ScoreRowV1. */
export const scoreExecution = (input: ScoreInput): ScoreRow => {
  if (input.record.run.caseId !== input.case.id) {
    throw new TypeError('execution record and case identifiers differ');
  }
  validateEligibilityPolicy(input);
  const success = successful(input);
  const observations = input.evidence.observations
    ? scoreObservations(input.evidence.observations)
    : null;
  return ScoreRowV1.parse({
    schemaVersion: 1,
    runId: input.record.run.id,
    attempt: input.record.attempt,
    studyId: input.record.run.studyId,
    phase: input.record.run.phase,
    caseId: input.case.id,
    familyId: input.case.familyId,
    conditionId: input.record.run.conditionId,
    repetition: input.record.run.repetition,
    pairedBlock: input.record.run.pairedBlock,
    provider: input.record.run.model.provider,
    model: input.record.run.model.model,
    effort: input.record.run.model.effort,
    freezeHash: input.record.run.freezeHash,
    traceRootHash: input.record.trace.rootHash,
    traceDerivedHash: input.record.trace.derivedHash,
    evidenceHash: artifactHash({
      caseHash: input.case.contentHash,
      recordHash: artifactHash(input.record),
      traceDerivedHash: input.record.trace.derivedHash,
      evidence: input.evidence,
    }),
    worldHash: input.record.worldHash,
    modelCallBudget: input.record.run.modelCallBudget ?? null,
    domain: input.case.domain,
    compositionClass: input.case.compositionClass,
    adaptive: input.case.adaptive,
    success: success ? 1 : 0,
    primaryEligible: primaryEligible(input),
    infrastructure: input.record.status === 'infrastructure',
    failureCode: failureCode(input, success),
    retrieval: retrieval(input),
    bundleExact: input.evidence.selectedSkills
      ? exactOrder(
          input.evidence.selectedSkills,
          input.case.gold.requiredSkills,
        )
      : null,
    menuExact:
      input.evidence.actualMenu && input.evidence.expectedMenu
        ? exactOrder(input.evidence.actualMenu, input.evidence.expectedMenu)
        : null,
    observationExact: observations?.exact ?? null,
    inputTokens: input.record.usage.inputTokens,
    outputTokens: input.record.usage.outputTokens,
    modelCalls: input.record.usage.modelCalls,
    toolCalls: input.record.usage.toolCalls,
    costUsd: input.record.usage.costUsd,
    durationMs: input.record.durationMs,
  });
};
