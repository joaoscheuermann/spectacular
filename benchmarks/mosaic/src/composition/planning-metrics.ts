import {
  type PlanningCondition,
  type PlanningConditionResult,
  planningConditions,
} from './planning-runner.js';
import type {
  CompositionClass,
  PlanningCase,
  PlanningDomain,
} from './planning-schema.js';
import type { PlanningTransitionScore } from './planning-scoring.js';

export interface PlanningCaseRun {
  readonly case: Pick<PlanningCase, 'id' | 'domain' | 'compositionClass'>;
  readonly results: readonly PlanningMetricResult[];
}

export type PlanningMetricResult = Pick<
  PlanningConditionResult,
  'condition' | 'evidenceSkillIds'
> & { readonly score: PlanningTransitionScore };

export interface PlanningConditionMetrics {
  readonly condition: PlanningCondition;
  readonly caseCount: number;
  readonly meanP0Score: number;
  readonly meanP0AgainstP1Score: number;
  readonly meanP1Score: number;
  readonly meanP1RetentionScore: number;
  readonly meanGain: number;
  readonly p0PassRate: number;
  readonly p1PassRate: number;
  readonly transitionPassRate: number;
  readonly regressionRate: number;
  readonly meanEvidenceSkills: number;
}

export interface PlanningAggregateMetrics {
  readonly caseCount: number;
  readonly conditions: readonly PlanningConditionMetrics[];
  readonly byDomain: Readonly<
    Partial<Record<PlanningDomain, readonly PlanningConditionMetrics[]>>
  >;
  readonly byCompositionClass: Readonly<
    Partial<Record<CompositionClass, readonly PlanningConditionMetrics[]>>
  >;
}

/** Macro-averages controlled planning transitions overall and by matrix cell. */
export const aggregatePlanningRuns = (
  runs: readonly PlanningCaseRun[],
): PlanningAggregateMetrics => {
  if (runs.length === 0)
    {throw new Error('Controlled planning metrics require at least one case.');}

  assertUnique(runs.map(({ case: benchmarkCase }) => benchmarkCase.id));

  runs.forEach(({ case: benchmarkCase, results }) => {
    if (results.length === 0)
      {throw new Error(
        `Planning case has no condition results: ${benchmarkCase.id}`,
      );}

    assertUnique(
      results.map(({ condition }) => condition),
      `condition in ${benchmarkCase.id}`,
    );
  });

  return {
    caseCount: runs.length,
    conditions: summarize(runs),
    byDomain: group(runs, ({ case: benchmarkCase }) => benchmarkCase.domain),
    byCompositionClass: group(
      runs,
      ({ case: benchmarkCase }) => benchmarkCase.compositionClass,
    ),
  };
};

const group = <Key extends string>(
  runs: readonly PlanningCaseRun[],
  key: (run: PlanningCaseRun) => Key,
): Readonly<Partial<Record<Key, readonly PlanningConditionMetrics[]>>> =>
  Object.fromEntries(
    [...new Set(runs.map(key))]
      .sort()
      .map((value) => [
        value,
        summarize(runs.filter((run) => key(run) === value)),
      ]),
  ) as Readonly<Partial<Record<Key, readonly PlanningConditionMetrics[]>>>;

const summarize = (
  runs: readonly PlanningCaseRun[],
): readonly PlanningConditionMetrics[] =>
  planningConditions.flatMap((condition) => {
    const results = runs.flatMap(({ results }) =>
      results.filter((result) => result.condition === condition),
    );

    if (results.length === 0) {return [];}

    return [
      {
        condition,
        caseCount: results.length,
        meanP0Score: mean(results.map(({ score }) => score.p0.score)),
        meanP0AgainstP1Score: mean(
          results.map(({ score }) => score.p0AgainstP1.score),
        ),
        meanP1Score: mean(results.map(({ score }) => score.p1.score)),
        meanP1RetentionScore: mean(
          results.map(({ score }) => score.p1Retention.score),
        ),
        meanGain: mean(results.map(({ score }) => score.gain)),
        p0PassRate: mean(results.map(({ score }) => Number(score.p0.passed))),
        p1PassRate: mean(results.map(({ score }) => Number(score.p1.passed))),
        transitionPassRate: mean(
          results.map(({ score }) => Number(score.passed)),
        ),
        regressionRate: mean(
          results.map(({ score }) => Number(score.regressions.length > 0)),
        ),
        meanEvidenceSkills: mean(
          results.map(({ evidenceSkillIds }) => evidenceSkillIds.length),
        ),
      },
    ];
  });

const mean = (values: readonly number[]): number =>
  Number(
    (values.reduce((total, value) => total + value, 0) / values.length).toFixed(
      12,
    ),
  );

const assertUnique = (values: readonly string[], label = 'case'): void => {
  if (new Set(values).size !== values.length)
    {throw new Error(`Duplicate planning ${label} identifier.`);}
};
