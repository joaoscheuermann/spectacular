import assert from 'node:assert/strict';
import test from 'node:test';

import {
  type CompositionClass,
  planningCase,
  planningCases,
  PlanningCaseSchema,
  type PlanningDomain,
  planningPhaseAtoms,
} from '../src/composition/planning.js';

const domains: readonly PlanningDomain[] = [
  'documents-finance',
  'software',
  'artifacts',
  'communications',
];
const classes: readonly CompositionClass[] = ['A', 'B', 'C', 'D', 'E', 'F'];

test('provides one valid case for every class and domain cell', () => {
  assert.equal(planningCases.length, 24);

  assert.equal(new Set(planningCases.map(({ id }) => id)).size, 24);

  assert.equal(new Set(planningCases.map(({ request }) => request)).size, 24);

  for (const domain of domains) {
    for (const compositionClass of classes) {
      const matches = planningCases.filter(
        (candidate) =>
          candidate.domain === domain &&
          candidate.compositionClass === compositionClass,
      );

      assert.equal(matches.length, 1, `${domain}/${compositionClass}`);

      assert.deepEqual(PlanningCaseSchema.parse(matches[0]), matches[0]);
    }
  }
});

test('keeps catalog relevance and distractors explicit in every case', () => {
  for (const benchmarkCase of planningCases) {
    assert.ok(benchmarkCase.catalog.length >= 6);

    assert.ok(benchmarkCase.gold.distractorSkillIds.length >= 2);

    assert.equal(
      benchmarkCase.gold.relevantSkillIds.length +
        benchmarkCase.gold.distractorSkillIds.length,
      benchmarkCase.catalog.length,
    );

    assert.deepEqual(
      benchmarkCase.catalog
        .filter(({ relevance }) => relevance === 'relevant')
        .map(({ id }) => id)
        .sort(),
      [...benchmarkCase.gold.relevantSkillIds].sort(),
    );
  }
});

test('preserves P0 requirements and adds catalog behaviors only to skill-bearing P1 cases', () => {
  for (const benchmarkCase of planningCases) {
    const p0 = planningPhaseAtoms(benchmarkCase.criteria.p0);
    const p1 = planningPhaseAtoms(benchmarkCase.criteria.p1);

    assert.ok(
      [...p0].every((atom) => p1.has(atom)),
      benchmarkCase.id,
    );

    if (
      benchmarkCase.compositionClass === 'A' ||
      benchmarkCase.compositionClass === 'B'
    ) {
      assert.deepEqual([...p1].sort(), [...p0].sort(), benchmarkCase.id);

      assert.equal(benchmarkCase.gold.relevantSkillIds.length, 0);

      continue;
    }

    const catalogBehaviors = new Set(
      benchmarkCase.gold.behaviors
        .filter(({ source }) => source === 'catalog')
        .map(({ id }) => id),
    );
    const additions = [...p1].filter((atom) => !p0.has(atom));

    assert.ok(
      additions.some(
        (atom) =>
          atom.startsWith('behavior:') &&
          catalogBehaviors.has(atom.slice(atom.lastIndexOf(':') + 1)),
      ),
      benchmarkCase.id,
    );
  }
});

test('resolves cases by their stable ID and rejects unknown IDs', () => {
  const expected = planningCases[0];

  assert.strictEqual(planningCase(expected.id), expected);

  assert.throws(
    () => planningCase('planning.unknown.a'),
    /Unknown planning case/u,
  );
});
