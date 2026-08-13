import assert from 'node:assert/strict';
import test from 'node:test';

import {
  aggregatePlanningRuns,
  type PlanningCaseRun,
  type PlanningMetricResult,
} from '../src/composition/planning.js';

const result = (
  condition: PlanningMetricResult['condition'],
  p0: number,
  p0AgainstP1: number,
  p1: number,
  gain: number,
  regressions: readonly string[] = [],
): PlanningMetricResult =>
  ({
    condition,
    evidenceSkillIds: condition === 'no-hints' ? [] : ['skill.one'],
    score: {
      p0: { score: p0, passed: p0 === 1 },
      p0AgainstP1: { score: p0AgainstP1 },
      p1: { score: p1, passed: p1 === 1 },
      p1Retention: { score: regressions.length === 0 ? p0 : p0 / 2 },
      gain,
      regressions,
      passed: p0 === 1 && p1 === 1 && regressions.length === 0,
    },
  }) as unknown as PlanningMetricResult;

const runs: readonly PlanningCaseRun[] = [
  {
    case: {
      id: 'planning.software.c',
      domain: 'software',
      compositionClass: 'C',
    },
    results: [
      result('no-hints', 1, 0.5, 0.5, 0),
      result('gold', 1, 0.5, 1, 0.5),
    ],
  },
  {
    case: {
      id: 'planning.artifacts.c',
      domain: 'artifacts',
      compositionClass: 'C',
    },
    results: [
      result('no-hints', 0.5, 0.25, 0.25, 0),
      result('gold', 0.5, 0.25, 0.75, 0.5, ['role:lost']),
    ],
  },
];

test('reports P0-to-P1 gains and regressions overall and by stratum', () => {
  const metrics = aggregatePlanningRuns(runs);
  const gold = metrics.conditions.find(({ condition }) => condition === 'gold');

  assert.equal(metrics.caseCount, 2);
  assert.equal(gold?.meanGain, 0.5);
  assert.equal(gold?.meanP1Score, 0.875);
  assert.equal(gold?.regressionRate, 0.5);
  assert.equal(gold?.meanEvidenceSkills, 1);
  assert.equal(metrics.byDomain.software?.[1]?.caseCount, 1);
  assert.equal(metrics.byCompositionClass.C?.[1]?.caseCount, 2);
});

test('rejects empty and duplicate planning evidence', () => {
  assert.throws(() => aggregatePlanningRuns([]), /at least one case/);
  assert.throws(
    () => aggregatePlanningRuns([runs[0]!, runs[0]!]),
    /Duplicate planning case/,
  );
});
