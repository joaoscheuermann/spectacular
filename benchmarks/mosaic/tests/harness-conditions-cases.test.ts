import assert from 'node:assert/strict';
import test from 'node:test';

import {
  A1,
  A2,
  A3,
  A4,
  A5,
  B3,
  M0,
  M1,
  ORACLES,
  boundaryPolicy,
  validateAblationMatrix,
} from '../src/conditions/index.js';
import { artifactHash } from '../src/core/index.js';
import {
  DORIC_SMOKE_CASES,
  DORIC_SMOKE_CONDITIONS,
  PILOT_CASES,
  validateCases,
  validateConfirmatoryCases,
} from '../src/study/index.js';

test('pilot cases satisfy cardinality balance hashes and composition signatures', () => {
  assert.deepEqual(validateCases(PILOT_CASES), []);
});

test('composition classes enforce their required skill and tool cardinalities', () => {
  const counts = (value: 'A' | 'B' | 'C' | 'D' | 'E' | 'F') =>
    PILOT_CASES.filter((entry) => entry.compositionClass === value);
  assert.ok(
    counts('A').every(
      (entry) =>
        entry.gold.requiredSkills.length === 0 &&
        entry.gold.requiredTools.length === 0,
    ),
  );
  assert.ok(
    counts('B').every(
      (entry) =>
        entry.gold.requiredSkills.length === 0 &&
        entry.gold.requiredTools.length >= 1,
    ),
  );
  assert.ok(
    counts('C').every(
      (entry) =>
        entry.gold.requiredSkills.length >= 1 &&
        entry.gold.requiredTools.length === 0,
    ),
  );
  assert.ok(
    counts('D').every(
      (entry) =>
        entry.gold.requiredSkills.length === 1 &&
        entry.gold.requiredTools.length >= 1,
    ),
  );
  assert.ok(
    counts('E').every(
      (entry) =>
        entry.gold.requiredSkills.length >= 2 &&
        entry.gold.requiredTools.length <= 1,
    ),
  );
  assert.ok(
    counts('F').every(
      (entry) =>
        entry.gold.requiredSkills.length >= 2 &&
        entry.gold.requiredTools.length >= 2,
    ),
  );
  assert.ok(
    [...counts('A'), ...counts('B')].every(
      (entry) => entry.gold.forbiddenSkills.length === 60,
    ),
  );
  assert.ok(
    PILOT_CASES.filter((entry) => entry.gold.requiresRevision).every(
      (entry) =>
        entry.gold.requiredTools.length > 0 &&
        entry.request.includes('structural invalidation'),
    ),
  );
});

test('M0 is selective MOSAIC without feedback while B3 uses a fixed top-three bundle', () => {
  assert.equal(M0.factors.catalogFeedback, false);
  assert.equal(M0.factors.bundle, 'selective');
  assert.equal(B3.factors.catalogFeedback, false);
  assert.equal(B3.factors.bundle, 'top-k');
  assert.equal(boundaryPolicy(M0).feedbackPlan, 'unchanged');
  assert.equal(boundaryPolicy(M1).feedbackPlan, 'default');
});

test('ablations change metadata order base tools selection and revision independently', () => {
  assert.deepEqual(validateAblationMatrix(), []);
  assert.equal(A1.factors.skillView, 'metadata');
  assert.equal(A2.factors.bundleOrder, 'shuffled');
  assert.equal(A3.factors.baseTools, false);
  assert.equal(A4.factors.bundle, 'top-k');
  assert.equal(A5.factors.localizedRevision, false);
});

test('diagnostic oracles are failures-only and map to explicit boundaries', () => {
  assert.ok(ORACLES.every((entry) => entry.eligibility === 'failures-only'));
  assert.equal(
    boundaryPolicy(ORACLES.find((entry) => entry.id === 'O_PLAN')!).initialPlan,
    'oracle',
  );
  assert.equal(
    boundaryPolicy(ORACLES.find((entry) => entry.id === 'O_STATE')!).execution,
    'oracle-state',
  );
  assert.equal(
    boundaryPolicy(ORACLES.find((entry) => entry.id === 'O_REVISION')!)
      .localizedRevision,
    'oracle',
  );
});

test('Doric smokes remain exactly six opt-in non-primary cases', () => {
  assert.equal(DORIC_SMOKE_CASES.length, 6);
  assert.equal(DORIC_SMOKE_CONDITIONS.length, 6);
  assert.deepEqual(
    new Set(DORIC_SMOKE_CASES.map((entry) => entry.compositionClass)),
    new Set(['A', 'B', 'C', 'D', 'E', 'F']),
  );
  assert.ok(DORIC_SMOKE_CASES.every((entry) => entry.phase === 'smoke'));
  assert.ok(
    DORIC_SMOKE_CONDITIONS.every((entry) => entry.eligibility === 'opt-in'),
  );
});

test('confirmatory validator enforces independent balanced 6-by-4 cells', () => {
  const domains = [
    'documents-finance',
    'software',
    'artifacts',
    'communication',
  ] as const;
  const classes = ['A', 'B', 'C', 'D', 'E', 'F'] as const;
  const cases = classes.flatMap((compositionClass) =>
    domains.flatMap((domain) => {
      const source = PILOT_CASES.find(
        (entry) =>
          entry.compositionClass === compositionClass &&
          entry.domain === domain,
      )!;
      return Array.from({ length: 10 }, (_, index) => {
        const ordinal = `${compositionClass.toLocaleLowerCase('en-US')}.${domain}.${index + 1}`;
        const { contentHash: ignored, ...sourceBody } = source;
        void ignored;
        const body = {
          ...sourceBody,
          id: `confirmatory.case.${ordinal}`,
          familyId: `confirmatory.family.${ordinal}`,
          phase: 'confirmatory' as const,
          title: `Independent confirmatory ${ordinal}`,
          request: `${source.request}\n\nIndependent confirmatory scenario ${ordinal}.`,
          tags: [
            'confirmatory',
            `domain.${domain}`,
            `class.${compositionClass}`,
          ],
        };
        return { ...body, contentHash: artifactHash(body) };
      });
    }),
  );
  assert.equal(cases.length, 240);
  assert.deepEqual(validateConfirmatoryCases(cases, PILOT_CASES, 240), []);
  assert.ok(
    validateConfirmatoryCases(cases.slice(0, 120), PILOT_CASES, 120).some(
      ({ code }) => code === 'confirmatory_n_final',
    ),
  );
  assert.ok(
    validateConfirmatoryCases(
      [{ ...cases[0]!, familyId: PILOT_CASES[0]!.familyId }, ...cases.slice(1)],
      PILOT_CASES,
      240,
    ).some(({ code }) => code === 'family_leakage'),
  );

  const source = PILOT_CASES.find(
    (entry) =>
      entry.compositionClass === cases[0]!.compositionClass &&
      entry.domain === cases[0]!.domain,
  )!;
  const { contentHash: ignored, ...sourceBody } = source;
  void ignored;
  const cloneBody = {
    ...sourceBody,
    id: cases[0]!.id,
    familyId: cases[0]!.familyId,
    phase: 'confirmatory' as const,
    tags: cases[0]!.tags,
  };
  const cloned = { ...cloneBody, contentHash: artifactHash(cloneBody) };
  assert.ok(
    validateConfirmatoryCases(
      [cloned, ...cases.slice(1)],
      PILOT_CASES,
      240,
    ).some(({ code }) => code === 'pilot_content_leakage'),
  );
});
