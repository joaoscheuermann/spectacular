import assert from 'node:assert/strict';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import {
  materializeSkillsbenchCatalog,
  scanSkillsbenchCatalog,
  skillsbenchV1_1,
  stringifySkillsbenchCompositionManifest,
} from '../src/composition/skillsbench-catalog.js';

type FixtureSkill = {
  readonly directory: string;
  readonly name: string;
  readonly description: string;
  readonly body: string;
  readonly files?: Readonly<Record<string, string>>;
};

type FixtureTask = {
  readonly id: string;
  readonly skills: readonly FixtureSkill[];
};

const skill = (
  directory: string,
  name: string,
  description: string,
  body: string,
  files: Readonly<Record<string, string>> = {},
): FixtureSkill => ({ directory, name, description, body, files });

const tasks = (): readonly FixtureTask[] => [
  {
    id: 'task-001',
    skills: [
      skill('shared-a', 'shared', 'Shared A', '# Shared A', {
        'scripts/run.sh': 'echo shared-a\n',
      }),
    ],
  },
  {
    id: 'task-002',
    skills: [
      skill('copy-of-shared-a', 'shared', 'Shared A', '# Shared A', {
        'scripts/run.sh': 'echo shared-a\n',
      }),
    ],
  },
  {
    id: 'task-003',
    skills: [skill('shared-b', 'shared', 'Shared B', '# Shared B')],
  },
  {
    id: 'task-004',
    skills: [skill('alpha-package', 'alpha', 'Alpha', '# Alpha')],
  },
  ...Array.from(
    { length: 83 },
    (_, index): FixtureTask => ({
      id: `task-${String(index + 5).padStart(3, '0')}`,
      skills: [skill('common-package', 'common', 'Common', '# Common')],
    }),
  ),
];

const writeSkill = async (
  root: string,
  taskId: string,
  value: FixtureSkill,
  reverseFiles: boolean,
): Promise<void> => {
  const directory = join(
    root,
    'tasks',
    taskId,
    'environment',
    'skills',
    value.directory,
  );

  await mkdir(directory, { recursive: true });

  await writeFile(
    join(directory, 'SKILL.md'),
    `---\nname: ${value.name}\ndescription: ${value.description}\n---\n${value.body}\n`,
    'utf8',
  );

  const files = Object.entries(value.files ?? {});

  if (reverseFiles) {files.reverse();}

  for (const [path, source] of files) {
    const target = join(directory, ...path.split('/'));

    await mkdir(join(target, '..'), { recursive: true });

    await writeFile(target, source, 'utf8');
  }
};

const checkout = async (reverse = false): Promise<string> => {
  const root = await mkdtemp(join(tmpdir(), 'skillsbench-composition-'));
  const definitions = [...tasks()];

  if (reverse) {definitions.reverse();}

  for (const task of definitions) {
    const directory = join(root, 'tasks', task.id);

    await mkdir(directory, { recursive: true });

    await writeFile(join(directory, 'task.md'), `# ${task.id}\n`, 'utf8');

    const taskSkills = [...task.skills];

    if (reverse) {taskSkills.reverse();}

    for (const value of taskSkills) {
      await writeSkill(root, task.id, value, reverse);
    }
  }

  const ignored = join(root, 'tasks', 'task-004', 'environment', 'skills');

  await mkdir(join(ignored, 'licenses'), { recursive: true });

  await writeFile(join(ignored, 'reference.md'), 'task-local resource\n');

  return root;
};

const scan = (root: string) =>
  scanSkillsbenchCatalog({ root, revision: skillsbenchV1_1.revision });

test('builds a global catalog with collision-safe IDs and gold task associations', async (t) => {
  const root = await checkout();

  t.after(() => rm(root, { recursive: true, force: true }));

  const result = await scan(root);

  const shared = result.manifest.skills.filter(
    (entry) => entry.originalName === 'shared',
  );

  assert.deepEqual(result.manifest.source, skillsbenchV1_1);

  assert.deepEqual(result.manifest.counts, {
    tasks: 87,
    skillOccurrences: 87,
    catalogSkills: 4,
  });

  assert.equal(shared.length, 2);

  assert.equal(new Set(shared.map((entry) => entry.id)).size, 2);

  assert.equal(
    shared.every((entry) =>
      /^skillsbench--shared--[a-f0-9]{64}$/.test(entry.id),
    ),
    true,
  );

  assert.deepEqual(
    shared.map((entry) => entry.sources.length).sort((a, b) => a - b),
    [1, 2],
  );

  assert.deepEqual(
    result.manifest.tasks.find((task) => task.id === 'task-001')?.goldSkillIds,
    [shared.find((entry) => entry.description === 'Shared A')?.id],
  );

  assert.deepEqual(
    result.manifest.tasks.find((task) => task.id === 'task-003')?.goldSkillIds,
    [shared.find((entry) => entry.description === 'Shared B')?.id],
  );

  assert.deepEqual(result.skills.find((entry) => entry.id === 'alpha')?.skill, {
    name: 'alpha',
    description: 'Alpha',
    body: '# Alpha',
    allowedTools: ['terminal'],
    indexText: 'alpha | Alpha | terminal | # Alpha',
  });

  assert.match(result.manifest.catalogSha256, /^[a-f0-9]{64}$/);

  assert.match(result.manifest.manifestSha256, /^[a-f0-9]{64}$/);

  assert.deepEqual(
    JSON.parse(stringifySkillsbenchCompositionManifest(result.manifest)),
    result.manifest,
  );

  assert.equal(
    stringifySkillsbenchCompositionManifest(result.manifest).endsWith('\n'),
    true,
  );
});

test('produces the same manifest across checkout locations and creation order', async (t) => {
  const first = await checkout();
  const second = await checkout(true);

  t.after(() =>
    Promise.all(
      [first, second].map((root) => rm(root, { recursive: true, force: true })),
    ),
  );

  const [left, right] = await Promise.all([scan(first), scan(second)]);

  assert.deepEqual(left.manifest, right.manifest);

  assert.deepEqual(left.skills, right.skills);
});

test('materializes runtime bodies under task-neutral package paths', async (t) => {
  const root = await checkout();

  t.after(() => rm(root, { recursive: true, force: true }));

  const targetParent = await mkdtemp(join(tmpdir(), 'skillsbench-neutral-'));
  const targetRoot = join(targetParent, 'catalog');

  t.after(() => rm(targetParent, { recursive: true, force: true }));

  const catalog = await scan(root);
  const before = stringifySkillsbenchCompositionManifest(catalog.manifest);
  const skills = await materializeSkillsbenchCatalog(catalog, root, targetRoot);
  const alpha = skills.find((entry) => entry.name === 'alpha');
  const directory = join(targetRoot, 'alpha');

  assert.equal(
    alpha?.body,
    `Skill files are in ${directory}. Resolve scripts/, references/, and other relative paths from this directory.\n\n# Alpha`,
  );

  assert.equal(
    stringifySkillsbenchCompositionManifest(catalog.manifest),
    before,
  );

  assert.equal(alpha?.body.includes('task-004'), false);

  assert.equal(
    await readFile(join(directory, 'SKILL.md'), 'utf8'),
    '---\nname: alpha\ndescription: Alpha\n---\n# Alpha\n',
  );

  await assert.rejects(
    materializeSkillsbenchCatalog(catalog, root, join(root, 'neutral')),
    /must be outside/,
  );

  await assert.rejects(
    materializeSkillsbenchCatalog(catalog, root, targetRoot),
    /already exists/,
  );
});

test('changes package and catalog provenance when a supporting file changes', async (t) => {
  const root = await checkout();

  t.after(() => rm(root, { recursive: true, force: true }));

  const before = await scan(root);

  await writeFile(
    join(
      root,
      'tasks',
      'task-001',
      'environment',
      'skills',
      'shared-a',
      'scripts',
      'run.sh',
    ),
    'echo changed\n',
    'utf8',
  );

  const after = await scan(root);

  assert.notEqual(after.manifest.catalogSha256, before.manifest.catalogSha256);

  assert.notEqual(
    after.manifest.manifestSha256,
    before.manifest.manifestSha256,
  );

  assert.equal(after.manifest.counts.catalogSkills, 5);
});

test('rejects a revision or task roster outside the pinned v1.1 contract', async (t) => {
  await assert.rejects(
    scanSkillsbenchCatalog({ root: 'missing', revision: 'main' }),
    /pinned SkillsBench v1.1 revision/,
  );

  const root = await checkout();

  t.after(() => rm(root, { recursive: true, force: true }));

  await rm(join(root, 'tasks', 'task-087'), { recursive: true, force: true });

  await assert.rejects(scan(root), /exactly 87 task directories/);
});
