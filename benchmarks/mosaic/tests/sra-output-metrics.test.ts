import assert from 'node:assert/strict';
import test from 'node:test';

import {
  parseSraInferenceJsonl,
  parseSraSkillGoldsJson,
  scoreSraOutputSkills,
} from '../src/composition/sra-output-metrics.js';

const gold = parseSraSkillGoldsJson(
  JSON.stringify([
    {
      instance_id: 'champ-1',
      dataset: 'champ',
      gold_skill_ids: ['a', 'b'],
    },
    {
      instance_id: 'champ-2',
      dataset: 'champ',
      gold_skill_ids: ['c', 'd'],
    },
  ]),
);

const inference = parseSraInferenceJsonl(
  [
    {
      instance_id: 'champ-1',
      dataset: 'champ',
      skill_ids_used: ['a', 'b'],
      meta: { candidate_skill_ids: ['a', 'x', 'b'] },
    },
    {
      instance_id: 'champ-2',
      dataset: 'champ',
      skill_ids_used: ['x', 'c'],
      meta: { candidate_skill_ids: ['x', 'c', 'd'] },
    },
  ]
    .map((record) => JSON.stringify(record))
    .join('\n'),
);

test('scores the variable selected bundles independently of execution output', () => {
  const result = scoreSraOutputSkills(inference, gold, 'selected', 8);

  assert.equal(result.dataset, 'champ');
  assert.equal(result.metrics.queryCount, 2);
  assert.equal(result.metrics.recallAtK, 0.75);
  assert.equal(result.metrics.setPrecisionAtK, 0.75);
  assert.equal(result.metrics.exactMatchRate, 0.5);
  assert.equal(result.metrics.cardinalityAccuracy, 1);
});

test('scores the frozen candidate projection at an explicit cutoff', () => {
  const result = scoreSraOutputSkills(inference, gold, 'candidates', 2);

  assert.equal(result.metrics.recallAtK, 0.5);
  assert.equal(result.metrics.setPrecisionAtK, 0.5);
});

test('rejects incomplete, duplicate, and cross-dataset evidence', () => {
  assert.throws(
    () => scoreSraOutputSkills(inference.slice(0, 1), gold, 'selected', 8),
    /instance sets differ/,
  );
  assert.throws(
    () =>
      scoreSraOutputSkills(
        [{ ...inference[0]!, skill_ids_used: ['a', 'a'] }, inference[1]!],
        gold,
        'selected',
        8,
      ),
    /Duplicate selected skill/,
  );
  assert.throws(
    () =>
      scoreSraOutputSkills(
        [{ ...inference[0]!, dataset: 'other' }, inference[1]!],
        gold,
        'selected',
        8,
      ),
    /one dataset|dataset differs/,
  );
});
