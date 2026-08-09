import assert from 'node:assert/strict';
import test from 'node:test';

import type { Case } from '../src/schemas/index.js';
import {
  PILOT_CASES,
  compileCalibrationCases,
  validateCalibrationCorpus,
} from '../src/study/index.js';

const draftFrom = (entry: Case, index: number) => {
  const document = entry.gold.expectedDelivery.document;
  return {
    schemaVersion: 1,
    id: `calibration.case.${String(index + 1).padStart(2, '0')}`,
    familyId: `calibration.family.${String(index + 1).padStart(2, '0')}`,
    phase: 'calibration',
    title: `Calibration scenario ${index + 1}: ${entry.title}`,
    domain: entry.domain,
    compositionClass: entry.compositionClass,
    focusGoalRole: entry.focusGoalRole,
    adaptive: entry.adaptive,
    request: `${entry.request}\n\nIndependently authored calibration scenario ${index + 1}.`,
    fixtureIds: entry.fixtureIds,
    tags: [
      'calibration',
      `domain.${entry.domain}`,
      `class.${entry.compositionClass}`,
    ],
    criteria: entry.gold.criteria,
    requiredSkills: entry.gold.requiredSkills,
    relevantSkills: entry.gold.relevantSkills,
    forbiddenSkills: entry.gold.forbiddenSkills,
    tools: entry.gold.expectedState.toolEvidence.map(({ id, name, input }) => ({
      id,
      name,
      input,
    })),
    expectedAnswer: document['answer'],
    requiresRevision: entry.gold.requiresRevision,
  };
};

const drafts = () => ({
  schemaVersion: 1,
  cases: PILOT_CASES.map(draftFrom),
});

test('compiles a balanced calibration corpus and derives every deterministic hash', () => {
  const report = compileCalibrationCases(drafts(), PILOT_CASES);

  assert.equal(report.valid, true);
  assert.equal(report.cases.length, 60);
  assert.deepEqual(report.issues, []);
  assert.ok(
    report.cases.every(
      (entry) =>
        entry.phase === 'calibration' &&
        entry.gold.expectedState.toolEvidence.every((evidence) =>
          evidence.evidenceHash.startsWith('sha256:'),
        ) &&
        entry.contentHash.startsWith('sha256:'),
    ),
  );
  assert.deepEqual(
    report.cases.reduce<Record<string, number>>((counts, entry) => {
      counts[entry.domain] = (counts[entry.domain] ?? 0) + 1;
      counts[entry.compositionClass] =
        (counts[entry.compositionClass] ?? 0) + 1;
      return counts;
    }, {}),
    {
      'documents-finance': 15,
      software: 15,
      artifacts: 15,
      communication: 15,
      A: 10,
      B: 10,
      C: 10,
      D: 10,
      E: 10,
      F: 10,
    },
  );
});

test('reports every strict draft-schema issue instead of stopping at the first', () => {
  const input = drafts();
  const cases = input.cases.map((entry, index) =>
    index < 2 ? { ...entry, unexpected: true } : entry,
  );

  const report = compileCalibrationCases({ ...input, cases }, PILOT_CASES);

  assert.equal(report.valid, false);
  assert.equal(
    report.issues.filter(({ code }) => code === 'draft_schema').length,
    2,
  );
  assert.equal(report.cases.length, 0);
});

test('reports invalid tool execution balance and phase issues together', () => {
  const input = drafts();
  const cases = input.cases.map((entry, index) => {
    if (index === 1) {
      return {
        ...entry,
        tools: [{ id: 'tool.invalid', name: 'missing-tool', input: {} }],
      };
    }
    return index === 2 ? { ...entry, phase: 'pilot' } : entry;
  });

  const report = compileCalibrationCases({ ...input, cases }, PILOT_CASES);

  assert.equal(report.valid, false);
  assert.ok(report.issues.some(({ code }) => code === 'unknown_tool'));
  assert.ok(
    report.issues.some(({ code }) => code === 'calibration_cardinality'),
  );
  assert.ok(report.issues.some(({ code }) => code === 'unbalanced_dimension'));
  assert.ok(report.issues.some(({ code }) => code === 'calibration_phase'));
});

test('standalone validation aggregates schema phase and hash failures', () => {
  const compiled = compileCalibrationCases(drafts(), PILOT_CASES);
  assert.equal(compiled.valid, true);
  const cases: unknown[] = compiled.cases.map((entry, index) =>
    index === 0 ? { ...entry, phase: 'pilot' } : entry,
  );
  cases.push({ schemaVersion: 1, id: 'malformed' });

  const report = validateCalibrationCorpus(cases, PILOT_CASES);

  assert.equal(report.valid, false);
  assert.ok(report.issues.some(({ code }) => code === 'case_schema'));
  assert.ok(report.issues.some(({ code }) => code === 'calibration_phase'));
  assert.ok(report.issues.some(({ code }) => code === 'content_hash'));
  assert.deepEqual(report.humanAuditRequired, [
    'semantic-neutrality',
    'cross-phase-independence',
  ]);
});
