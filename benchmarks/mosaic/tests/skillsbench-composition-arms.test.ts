import assert from 'node:assert/strict';
import test from 'node:test';

import {
  defineSkillsbenchComposition,
  resolveSkillsbenchCompositionArm,
} from '../src/composition/skillsbench-arms.js';
import {
  type SkillsbenchCompositionManifest,
  skillsbenchV1_1,
} from '../src/composition/skillsbench-catalog.js';

const sha = (character: string): string => character.repeat(64);

const manifest: SkillsbenchCompositionManifest = {
  schemaVersion: 1,
  condition: 'skillsbench-composition',
  source: skillsbenchV1_1,
  counts: { tasks: 2, skillOccurrences: 3, catalogSkills: 4 },
  catalogSha256: sha('b'),
  skills: ['alpha', 'bravo', 'charlie', 'delta'].map((id) => ({
    id,
    originalName: id,
    description: `${id} skill`,
    packageSha256: sha(id === 'alpha' ? '1' : id === 'bravo' ? '2' : '3'),
    files: [{ path: 'SKILL.md', bytes: 1, sha256: sha('4') }],
    sources: [
      {
        taskId: 'task-a',
        directory: id,
        path: `tasks/task-a/environment/skills/${id}`,
      },
    ],
  })),
  tasks: [
    { id: 'task-a', goldSkillIds: ['alpha', 'bravo'] },
    { id: 'task-b', goldSkillIds: ['charlie'] },
  ],
  manifestSha256: sha('c'),
};

const ranking = {
  catalogSha256: manifest.catalogSha256,
  ranker: { id: 'tfidf-unigram-bigram', revision: 'fixture-v1' },
  tasks: [
    {
      id: 'task-a',
      skillIds: ['delta', 'charlie', 'bravo', 'alpha'],
    },
    {
      id: 'task-b',
      skillIds: ['alpha', 'bravo', 'charlie', 'delta'],
    },
  ],
} as const;

test('defines five arms and marks oracle and all-skills diagnostic', () => {
  const contract = defineSkillsbenchComposition(manifest, ranking);

  assert.deepEqual(
    contract.arms.map((arm) => arm.id),
    ['no-skills', 'fixed-top-k', 'mosaic-selective', 'oracle', 'all-skills'],
  );

  assert.deepEqual(
    contract.arms.filter((arm) => arm.diagnosticOnly).map((arm) => arm.id),
    ['oracle', 'all-skills'],
  );

  assert.deepEqual(contract.arms[1]?.selection, {
    kind: 'fixed-top-k',
    k: 3,
    rankingSha256: contract.fixedRanking.sha256,
  });

  assert.deepEqual(contract.arms[2]?.selection, {
    kind: 'mosaic-selective',
    maxSkills: 8,
  });

  assert.equal(contract.catalogSha256, manifest.catalogSha256);

  assert.equal(contract.manifestSha256, manifest.manifestSha256);

  assert.match(contract.fixedRanking.sha256, /^[a-f0-9]{64}$/);
});

test('resolves static, ranked, gold, all, and selective skill assignments', () => {
  const contract = defineSkillsbenchComposition(manifest, ranking);

  assert.deepEqual(
    resolveSkillsbenchCompositionArm({
      contract,
      manifest,
      arm: 'no-skills',
      taskId: 'task-a',
    }),
    { kind: 'preloaded', skillIds: [] },
  );

  assert.deepEqual(
    resolveSkillsbenchCompositionArm({
      contract,
      manifest,
      arm: 'fixed-top-k',
      taskId: 'task-a',
    }),
    { kind: 'preloaded', skillIds: ['delta', 'charlie', 'bravo'] },
  );

  assert.deepEqual(
    resolveSkillsbenchCompositionArm({
      contract,
      manifest,
      arm: 'oracle',
      taskId: 'task-a',
    }),
    { kind: 'preloaded', skillIds: ['alpha', 'bravo'] },
  );

  assert.deepEqual(
    resolveSkillsbenchCompositionArm({
      contract,
      manifest,
      arm: 'all-skills',
      taskId: 'task-a',
    }),
    {
      kind: 'preloaded',
      skillIds: ['alpha', 'bravo', 'charlie', 'delta'],
    },
  );

  assert.deepEqual(
    resolveSkillsbenchCompositionArm({
      contract,
      manifest,
      arm: 'mosaic-selective',
      taskId: 'task-a',
    }),
    {
      kind: 'selective',
      candidateSkillIds: ['alpha', 'bravo', 'charlie', 'delta'],
      maxSkills: 8,
    },
  );
});

test('rejects incomplete, duplicate, unknown, or mismatched fixed rankings', () => {
  assert.throws(
    () =>
      defineSkillsbenchComposition(manifest, {
        ...ranking,
        catalogSha256: sha('d'),
      }),
    /catalog digest/,
  );

  assert.throws(
    () =>
      defineSkillsbenchComposition(manifest, {
        ...ranking,
        tasks: ranking.tasks.slice(0, 1),
      }),
    /one complete ranking per task/,
  );

  assert.throws(
    () =>
      defineSkillsbenchComposition(manifest, {
        ...ranking,
        tasks: [
          { id: 'task-a', skillIds: ['alpha', 'alpha', 'charlie', 'delta'] },
          ranking.tasks[1],
        ],
      }),
    /complete catalog permutation/,
  );

  assert.throws(
    () =>
      resolveSkillsbenchCompositionArm({
        contract: defineSkillsbenchComposition(manifest, ranking),
        manifest,
        arm: 'oracle',
        taskId: 'unknown-task',
      }),
    /Unknown SkillsBench task/,
  );
});
