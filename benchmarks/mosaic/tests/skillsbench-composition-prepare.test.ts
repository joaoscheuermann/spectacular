import assert from 'node:assert/strict';
import { mkdtemp, readdir, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import type { SkillsbenchCatalog } from '../src/composition/skillsbench-catalog.js';
import {
  buildSkillsbenchFixedRanking,
  type SkillsbenchCommandRunner,
  skillsbenchCompositionPin,
  verifySkillsbenchCheckout,
  writeSkillsbenchPreparation,
} from '../src/composition/skillsbench-prepare.js';

test('pins the preflighted canonical v1.1 catalog and ranking identities', () => {
  assert.deepEqual(skillsbenchCompositionPin, {
    skillOccurrences: 232,
    catalogSkills: 209,
    catalogSha256:
      '382379cc8b2ac56aab6d6c4559bb2e6b1d6203f5de58534532e9ad528bdefe7c',
    rankingSha256:
      '0a1631e7ad74730c909194efee941d2ba981279cd8d040941d2a5039fafc9ffd',
  });
});

const sha = (value: string): string => value.repeat(64);

const catalog = (golds: readonly string[] = ['alpha']): SkillsbenchCatalog => ({
  manifest: {
    schemaVersion: 1,
    condition: 'skillsbench-composition',
    source: {
      repository: 'benchflow-ai/skillsbench',
      revision: 'b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af',
      tasksPath: 'tasks',
      taskCount: 87,
    },
    counts: { tasks: 2, skillOccurrences: 2, catalogSkills: 3 },
    catalogSha256: sha('a'),
    skills: [],
    tasks: [
      { id: 'task-one', goldSkillIds: golds },
      { id: 'task-two', goldSkillIds: ['gamma'] },
    ],
    manifestSha256: sha('b'),
  },
  skills: [
    runtimeSkill('alpha', 'recover spreadsheet formulas'),
    runtimeSkill('beta', 'format a presentation deck'),
    runtimeSkill('gamma', 'recover spreadsheet formulas and values'),
  ],
});

test('ranks full catalog permutations without consulting task golds', async () => {
  const prompts = new Map([
    ['task-one', 'Recover spreadsheet formulas and values.'],
    ['task-two', 'Format a presentation deck.'],
  ]);
  const readTask = async (id: string) => prompts.get(id)!;

  const first = await buildSkillsbenchFixedRanking(
    catalog(['alpha']),
    readTask,
  );

  const changedGold = await buildSkillsbenchFixedRanking(
    catalog(['beta', 'gamma']),
    readTask,
  );

  assert.deepEqual(changedGold, first);

  assert.deepEqual(first.tasks[0]?.skillIds, ['gamma', 'alpha', 'beta']);

  assert.deepEqual(first.tasks[1]?.skillIds, ['beta', 'alpha', 'gamma']);

  assert.ok(
    first.tasks.every(
      ({ skillIds }) =>
        skillIds.length === 3 && new Set(skillIds).size === skillIds.length,
    ),
  );
});

test('verifies exact Git root, revision, cleanliness, and file modes', async () => {
  const root = await mkdtemp(join(tmpdir(), 'skillsbench-git-'));
  const calls: readonly string[][] = [];
  const mutableCalls = calls as string[][];

  const runner: SkillsbenchCommandRunner = {
    run: async (_command, args) => {
      mutableCalls.push([...args]);

      const operation = args.slice(2).join(' ');

      if (operation === 'rev-parse --show-toplevel')
        {return { stdout: `${root}\n` };}

      if (operation === 'rev-parse HEAD')
        {return { stdout: 'b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af\n' };}

      return { stdout: '' };
    },
  };

  try {
    assert.equal(await verifySkillsbenchCheckout(root, runner), root);

    assert.equal(calls.length, 4);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('fails closed on dirty checkout and writes preparation exclusively', async () => {
  const root = await mkdtemp(join(tmpdir(), 'skillsbench-prepare-'));

  const dirty: SkillsbenchCommandRunner = {
    run: async (_command, args) => {
      const operation = args.slice(2).join(' ');

      if (operation === 'rev-parse --show-toplevel')
        {return { stdout: `${root}\n` };}

      if (operation === 'rev-parse HEAD')
        {return { stdout: 'b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af\n' };}

      return { stdout: '?? tasks/task-one/leak.txt\n' };
    },
  };
  const output = join(root, 'artifact.json');
  const preparation = { schemaVersion: 1, benchmark: 'fixture' } as never;

  try {
    await assert.rejects(
      verifySkillsbenchCheckout(root, dirty),
      /must be clean/,
    );

    await writeSkillsbenchPreparation(output, preparation);

    assert.deepEqual(JSON.parse(await readFile(output, 'utf8')), preparation);

    await assert.rejects(
      writeSkillsbenchPreparation(output, preparation),
      /EEXIST/,
    );

    assert.deepEqual((await readdir(root)).sort(), ['artifact.json']);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

const runtimeSkill = (id: string, text: string) => ({
  id,
  originalName: id,
  packageSha256: sha(id === 'alpha' ? '1' : id === 'beta' ? '2' : '3'),
  sources: [],
  skill: {
    name: id,
    description: text,
    body: text,
    allowedTools: ['terminal'] as const,
    indexText: `${id} | ${text}`,
  },
});
