import assert from 'node:assert/strict';
import test from 'node:test';

import type { ProviderProfile } from '../src/run.js';
import {
  runCompositionCommand,
  type CompositionCommandDependencies,
} from '../src/composition-cli.js';
import type { SkillsbenchPreparation } from '../src/composition/skillsbench-prepare.js';

const profile = {} as ProviderProfile;

test('dispatches network-free SRA pilot preparation', async () => {
  let received: unknown;
  const result = await runCompositionCommand(
    ['sra', 'prepare', '--source', '/source', '--output', '/output'],
    {
      prepareSra: async (options) => {
        received = options;
        return { schemaVersion: 1 } as never;
      },
    },
  );

  assert.deepEqual(received, {
    sourceRoot: '/source',
    outputDir: '/output',
  });
  assert.deepEqual(result, { schemaVersion: 1 });
});

test('prepares a global SkillsBench catalog without creating a provider', async () => {
  let prepared: unknown;
  let written: unknown;
  const preparation = {
    catalogManifest: {
      counts: { tasks: 87, catalogSkills: 196 },
      catalogSha256: 'a'.repeat(64),
    },
    contract: {
      fixedRanking: { sha256: 'b'.repeat(64) },
      arms: [{ id: 'no-skills' }, { id: 'mosaic-selective' }],
    },
  } as unknown as SkillsbenchPreparation;
  const result = await runCompositionCommand(
    [
      'skillsbench',
      'prepare',
      '--source',
      '/checkout',
      '--output',
      '/artifact.json',
    ],
    {
      prepareSkillsbench: async (options) => {
        prepared = options;
        return preparation;
      },
      writeSkillsbench: async (...args) => {
        written = args;
      },
      createProfile: () => {
        throw new Error('must not create a provider');
      },
    },
  );

  assert.deepEqual(prepared, { sourceRoot: '/checkout' });
  assert.deepEqual(written, ['/artifact.json', preparation]);
  assert.deepEqual(result, {
    benchmark: 'SkillsBench Composition',
    outputPath: '/artifact.json',
    tasks: 87,
    catalogSkills: 196,
    catalogSha256: 'a'.repeat(64),
    rankingSha256: 'b'.repeat(64),
    arms: ['no-skills', 'mosaic-selective'],
  });
});

test('describes the controlled planning matrix without a provider', async () => {
  let called = false;
  const result = await runCompositionCommand(['planning', 'manifest'], {
    planningManifest: () => {
      called = true;
      return { benchmark: 'mosaic-p0-p1-controlled' } as never;
    },
    createProfile: () => {
      throw new Error('must not create a provider');
    },
  });

  assert.equal(called, true);
  assert.deepEqual(result, { benchmark: 'mosaic-p0-p1-controlled' });
});

test('requires paid approval before a controlled planning run', async () => {
  let called = false;
  await assert.rejects(
    runCompositionCommand(
      ['planning', 'run', '--output', '/output', '--case', 'planning.a'],
      {
        createProfile: () => {
          called = true;
          return profile;
        },
      },
    ),
    /yes-paid-run/,
  );
  assert.equal(called, false);
});

test('dispatches selected controlled planning cases after approval', async () => {
  let received: unknown;
  const result = await runCompositionCommand(
    [
      'planning',
      'run',
      '--output',
      '/output',
      '--case',
      'planning.a',
      '--case',
      'planning.b',
      '--max-turns',
      '4',
      '--yes-paid-run',
    ],
    {
      createProfile: () => profile,
      runPlanning: async (options) => {
        received = options;
        return { caseCount: 2 } as never;
      },
    },
  );

  assert.deepEqual(received, {
    profile,
    outputDir: '/output',
    caseIds: ['planning.a', 'planning.b'],
    maxTurns: 4,
  });
  assert.deepEqual(result, { caseCount: 2 });
});

test('scores an official retrieval artifact without a provider', async () => {
  let received: unknown;
  const result = await runCompositionCommand(
    [
      'sra',
      'score',
      '--input',
      '/retrieval.json',
      '--k',
      '5',
      '--output',
      '/metrics.json',
    ],
    {
      scoreSra: async (...args) => {
        received = args;
        return { k: 5 } as never;
      },
    },
  );

  assert.deepEqual(received, ['/retrieval.json', 5, '/metrics.json']);
  assert.deepEqual(result, { k: 5 });
});

test('scores selected SRA bundles without a provider', async () => {
  let received: unknown;
  const result = await runCompositionCommand(
    [
      'sra',
      'score-output',
      '--input',
      '/inference.jsonl',
      '--gold',
      '/gold.json',
      '--projection',
      'selected',
      '--k',
      '8',
      '--output',
      '/selection.json',
    ],
    {
      scoreSraOutput: async (...args) => {
        received = args;
        return { projection: 'selected' } as never;
      },
    },
  );

  assert.deepEqual(received, [
    '/inference.jsonl',
    '/gold.json',
    'selected',
    8,
    '/selection.json',
  ]);
  assert.deepEqual(result, { projection: 'selected' });
});

test('rejects a paid SRA run before reading inputs or creating a provider', async () => {
  let called = false;
  const dependencies: CompositionCommandDependencies = {
    readFile: async () => {
      called = true;
      return '';
    },
    createProfile: () => {
      called = true;
      return profile;
    },
  };

  await assert.rejects(
    () =>
      runCompositionCommand(
        [
          'sra',
          'run',
          '--arm',
          'oracle',
          '--instances',
          '/instances.json',
          '--corpus',
          '/corpus.json',
          '--output',
          '/output.jsonl',
        ],
        dependencies,
      ),
    /yes-paid-run/,
  );
  assert.equal(called, false);
});

test('dispatches an approved SRA arm with explicit frozen controls', async () => {
  const reads: string[] = [];
  let received: unknown;
  const result = await runCompositionCommand(
    [
      'sra',
      'run',
      '--arm',
      'mosaic',
      '--instances',
      '/instances.json',
      '--corpus',
      '/corpus.json',
      '--retrieval',
      '/retrieval.json',
      '--output',
      '/output.jsonl',
      '--max-hint-candidates',
      '6',
      '--max-retrieved-candidates',
      '50',
      '--max-skills',
      '6',
      '--yes-paid-run',
    ],
    {
      readFile: async (path) => {
        reads.push(path);
        if (path === '/instances.json')
          return JSON.stringify([
            {
              instance_id: 'champ_1',
              dataset: 'champ',
              question: 'Question?',
              skill_annotations: ['a', 'b'],
            },
          ]);
        if (path === '/corpus.json')
          return JSON.stringify(
            ['a', 'b'].map((id) => ({
              skill_id: id,
              name: id,
              description: id,
              content: id,
            })),
          );
        return JSON.stringify({
          results: [
            {
              instance_id: 'champ_1',
              gold_skill_ids: ['a', 'b'],
              retrieved: [
                { skill_id: 'a', score: 2 },
                { skill_id: 'b', score: 1 },
              ],
            },
          ],
        });
      },
      createProfile: () => profile,
      runSra: async (options) => {
        received = options;
        return { completed: 1 } as never;
      },
    },
  );

  assert.deepEqual(reads, [
    '/instances.json',
    '/corpus.json',
    '/retrieval.json',
  ]);
  assert.equal((received as { readonly arm: string }).arm, 'mosaic');
  assert.deepEqual(
    {
      maxHintCandidates: (received as { maxHintCandidates: number })
        .maxHintCandidates,
      maxRetrievedCandidates: (received as { maxRetrievedCandidates: number })
        .maxRetrievedCandidates,
      maxSkills: (received as { maxSkills: number }).maxSkills,
    },
    { maxHintCandidates: 6, maxRetrievedCandidates: 50, maxSkills: 6 },
  );
  assert.deepEqual(result, { completed: 1 });
});
