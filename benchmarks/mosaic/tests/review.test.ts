import assert from 'node:assert/strict';
import test from 'node:test';

import {
  prepareReview,
  reviewStatus,
  stability,
} from '../src/scoring/index.js';

const artifact = (condition: string, index: number) => ({
  hash: `sha256:${(condition === 'B2' ? index + 1 : index + 17)
    .toString(16)
    .padStart(64, '0')}`,
});

const cases = Array.from({ length: 10 }, (_, index) => ({
  caseId: `case-${String(index).padStart(2, '0')}`,
  cell: index < 5 ? 'A/documents' : 'B/software',
  repetitions: [1, 2, 3, 4, 5],
  artifacts: {
    B2: artifact('B2', index),
    M1: artifact('M1', index),
  },
}));

test('prepares exactly twenty percent of cases per cell for both conditions', () => {
  const prepared = prepareReview({
    studyId: 'study-1',
    cases,
    conditions: ['B2', 'M1'],
    seed: 'frozen-seed',
    rounds: ['2026-09-01T12:00:00.000Z', '2026-09-15T12:00:00.000Z'],
  });

  assert.equal(prepared.assignments.length, 8);
  assert.equal(prepared.key.length, 8);
  assert.equal(
    prepared.assignments.some((assignment) =>
      Object.hasOwn(assignment, 'conditionId'),
    ),
    false,
  );
  assert.equal(
    prepared.assignments.some(({ artifactPath }) => /B2|M1/.test(artifactPath)),
    false,
  );

  const counts = prepared.key.reduce<Record<string, number>>(
    (acc, item, index) => {
      const pass = prepared.assignments[index]?.pass;
      const key = `${item.cell}/${item.conditionId}/${pass}`;
      return { ...acc, [key]: (acc[key] ?? 0) + 1 };
    },
    {},
  );

  assert.deepEqual(Object.values(counts), Array(8).fill(1));
});

test('rejects review preparation without M1 and one frozen baseline', () => {
  assert.throws(
    () =>
      prepareReview({
        studyId: 'study-1',
        cases,
        conditions: ['M0', 'M1'],
        seed: 'frozen-seed',
        rounds: ['2026-09-01T12:00:00.000Z', '2026-09-15T12:00:00.000Z'],
      }),
    /frozen baseline/,
  );
});

test('rejects review rounds separated by less than fourteen days', () => {
  assert.throws(
    () =>
      prepareReview({
        studyId: 'study-1',
        cases,
        conditions: ['B2', 'M1'],
        seed: 'frozen-seed',
        rounds: ['2026-09-01T12:00:00.000Z', '2026-09-14T11:59:59.999Z'],
      }),
    /at least 14 days/,
  );
});

test('reports human test-retest stability and gates auxiliary extrapolation', () => {
  const prepared = prepareReview({
    studyId: 'study-1',
    cases,
    conditions: ['B2', 'M1'],
    seed: 'frozen-seed',
    rounds: ['2026-09-01T12:00:00.000Z', '2026-09-15T12:00:00.000Z'],
  });
  const subjects = [
    ...new Set(
      prepared.assignments.map(
        (assignment) =>
          `${assignment.caseId}/${assignment.conditionCode}/${assignment.repetition}`,
      ),
    ),
  ].sort();
  const acceptable = new Map(
    subjects.map((subject, index) => [subject, index % 2 === 0]),
  );
  const reviews = prepared.assignments.map((assignment) => ({
    schemaVersion: 1 as const,
    assignmentId: assignment.id,
    reviewerCode: 'reviewer-solo',
    submittedAt:
      assignment.pass === 1
        ? '2026-09-01T13:00:00.000Z'
        : '2026-09-15T13:00:00.000Z',
    acceptable:
      acceptable.get(
        `${assignment.caseId}/${assignment.conditionCode}/${assignment.repetition}`,
      ) ?? false,
    criteria: [{ criterionId: 'semantic', satisfied: true }],
    confidence: 4,
    notes: '',
    auxiliaryJudge: null,
  }));

  const result = reviewStatus(prepared, reviews);

  assert.equal(result.completedPassOne, 4);
  assert.equal(result.completedPassTwo, 4);
  assert.equal(result.stability, 1);
  assert.equal(result.judgeExtrapolationAllowed, true);
});

test('does not extrapolate the judge when human decisions have no variation', () => {
  const prepared = prepareReview({
    studyId: 'study-1',
    cases,
    conditions: ['B2', 'M1'],
    seed: 'frozen-seed',
    rounds: ['2026-09-01T12:00:00.000Z', '2026-09-15T12:00:00.000Z'],
  });
  const reviews = prepared.assignments.map((assignment) => ({
    schemaVersion: 1 as const,
    assignmentId: assignment.id,
    reviewerCode: 'reviewer-solo',
    submittedAt:
      assignment.pass === 1
        ? '2026-09-01T13:00:00.000Z'
        : '2026-09-15T13:00:00.000Z',
    acceptable: true,
    criteria: [{ criterionId: 'semantic', satisfied: true }],
    confidence: 4,
    notes: '',
    auxiliaryJudge: null,
  }));

  const result = reviewStatus(prepared, reviews);
  assert.equal(result.stability, null);
  assert.equal(result.judgeExtrapolationAllowed, false);
});

test('returns zero stability for systematic disagreement beyond chance', () => {
  assert.equal(
    stability([
      [false, false],
      [false, true],
      [true, false],
      [true, true],
    ]),
    0,
  );
});

test('preserves negative stability when disagreement is worse than chance', () => {
  assert.equal(
    stability([
      [false, true],
      [false, true],
      [true, false],
      [true, false],
    ]),
    -1,
  );
});
