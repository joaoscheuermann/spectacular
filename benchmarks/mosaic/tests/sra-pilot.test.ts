import assert from 'node:assert/strict';
import test from 'node:test';

import {
  parseSraCorpus,
  parseSraInstances,
} from '../src/composition/sra-fixtures.js';
import {
  buildSraSkillGolds,
  selectSraPilot,
  SRA_BENCH_MANIFEST,
} from '../src/composition/sra-pilot.js';

const makeInstance = (dataset: string, index: number, skillCount = 2) => ({
  instance_id: `${dataset}_${String(index).padStart(5, '0')}`,
  dataset,
  question: `${dataset} question ${index}`,
  skill_annotations: Array.from(
    { length: skillCount },
    (_, skillIndex) => `${dataset}_skill_${index}_${skillIndex}`,
  ),
});

const instances = parseSraInstances([
  ...SRA_BENCH_MANIFEST.pilot.strata.flatMap((stratum, stratumIndex) =>
    Array.from({ length: stratum.count + 2 }, (_, index) =>
      makeInstance(
        stratum.dataset,
        stratumIndex * 1_000 + index,
        stratum.cardinality,
      ),
    ),
  ),
  makeInstance('champ', 99, 1),
  makeInstance('logicbench', 0),
]);

test('pins the external benchmark and its balanced multi-skill pilot', () => {
  assert.deepEqual(SRA_BENCH_MANIFEST.upstream, {
    repository: 'https://github.com/oneal2000/SR-Agents',
    revision: '277fd8d2bbd7d3b81a5cf4ffa6e87e18c7906e4f',
    paper: 'https://arxiv.org/abs/2604.24594',
    license: 'MIT',
  });

  assert.equal(
    SRA_BENCH_MANIFEST.dataset.revision,
    '6143f2634eb284955ce312213bac24b582d039f3',
  );

  assert.deepEqual(SRA_BENCH_MANIFEST.pilot.datasets, [
    'champ',
    'bigcodebench',
  ]);

  assert.deepEqual(
    [
      SRA_BENCH_MANIFEST.pilot.minimumGoldSkills,
      SRA_BENCH_MANIFEST.pilot.perDataset,
      SRA_BENCH_MANIFEST.pilot.totalInstances,
    ],
    [2, 50, 100],
  );

  assert.equal(
    SRA_BENCH_MANIFEST.pilot.strata.reduce(
      (total, stratum) => total + stratum.count,
      0,
    ),
    100,
  );
});

test('selects the same balanced pilot regardless of fixture order', () => {
  const forward = selectSraPilot(instances);
  const reverse = selectSraPilot([...instances].reverse());

  assert.deepEqual(
    forward.map((instance) => instance.instance_id),
    reverse.map((instance) => instance.instance_id),
  );

  assert.deepEqual(
    forward.reduce<Record<string, number>>((counts, instance) => {
      counts[instance.dataset] = (counts[instance.dataset] ?? 0) + 1;

      return counts;
    }, {}),
    { champ: 50, bigcodebench: 50 },
  );

  assert.ok(
    forward.every((instance) => instance.skill_annotations.length >= 2),
  );

  assert.ok(forward.every((instance) => instance.dataset !== 'logicbench'));
});

test('rejects a missing cardinality stratum instead of shrinking the pilot', () => {
  assert.throws(
    () =>
      selectSraPilot(
        instances.filter(
          (instance) =>
            !(
              instance.dataset === 'champ' &&
              instance.skill_annotations.length === 5
            ),
        ),
      ),
    /requires 1 eligible champ cardinality-5 instances; found 0/,
  );
});

test('builds canonical gold skill sets and verifies them against the corpus', () => {
  const pilot = selectSraPilot(instances);

  const skillIds = new Set(
    pilot.flatMap((instance) => instance.skill_annotations),
  );

  const corpus = parseSraCorpus(
    [...skillIds].map((skillId) => ({
      skill_id: skillId,
      name: skillId,
      description: 'Synthetic skill.',
      content: 'Synthetic instructions.',
    })),
  );
  const golds = buildSraSkillGolds(pilot, corpus);

  assert.equal(golds.length, 100);

  assert.deepEqual(
    golds[0]?.gold_skill_ids,
    [...(pilot[0]?.skill_annotations ?? [])].sort(),
  );

  assert.throws(
    () => buildSraSkillGolds(pilot, corpus.slice(1)),
    /references missing corpus skill/,
  );
});
