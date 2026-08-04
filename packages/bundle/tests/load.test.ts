import assert from 'node:assert/strict';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import { loadBundles } from '../src/index.js';

type ToolEntry = { readonly path: string; readonly alwaysAvailable: boolean };
type SkillEntry = { readonly path: string; readonly alwaysAvailable: boolean };

const skill = (name: string, allowedTools: readonly string[] = []) => `---
name: ${name}
description: ${name} description
allowed-tools: [${allowedTools.join(', ')}]
---

# ${name}

Follow this procedure.
`;

const createRoot = () => mkdtemp(join(tmpdir(), 'bundle-test-'));

const createBundle = async (
  root: string,
  directory: string,
  options: {
    readonly name?: string;
    readonly tools?: readonly ToolEntry[];
    readonly skills?: readonly SkillEntry[];
  } = {},
) => {
  const path = join(root, directory);
  await Promise.all([
    mkdir(join(path, 'tools'), { recursive: true }),
    mkdir(join(path, 'skills'), { recursive: true }),
  ]);
  await writeFile(
    join(path, 'manifest.json'),
    JSON.stringify({
      name: options.name ?? directory,
      description: `${directory} description`,
      tools: options.tools ?? [],
      skills: options.skills ?? [],
    }),
  );
  return path;
};

const writeTool = async (bundle: string, file: string, name: string) =>
  writeFile(
    join(bundle, 'tools', file),
    `const definition = { name: ${JSON.stringify(name)}, inputSchema: {} };
    const factory = () => ({
      name: ${JSON.stringify(name)}, schema: {}, definition,
      execute: async (payload) => payload
    });
    Object.defineProperties(factory, {
      name: { value: ${JSON.stringify(name)} },
      schema: { value: {} },
      definition: { value: definition }
    });
    export default factory;`,
  );

test('preserves lexical bundle order and manifest resource order', async () => {
  const root = await createRoot();
  try {
    await createBundle(root, 'z');
    await createBundle(root, 'a');
    assert.deepEqual(
      (await loadBundles(root)).map(({ name }) => name),
      ['a', 'z'],
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('loads executable tools, availability flags, and skill frontmatter', async () => {
  const root = await createRoot();
  try {
    const bundle = await createBundle(root, 'core', {
      tools: [{ path: 'tools/example.js', alwaysAvailable: true }],
      skills: [{ path: 'skills/example/SKILL.md', alwaysAvailable: false }],
    });
    await writeTool(bundle, 'example.js', 'example');
    await mkdir(join(bundle, 'skills', 'example'));
    await writeFile(
      join(bundle, 'skills', 'example', 'SKILL.md'),
      skill('example-skill', ['example']),
    );

    const [loaded] = await loadBundles(root);
    assert.equal(loaded?.tools[0]?.alwaysAvailable, true);
    const factory = loaded?.tools[0]?.factory;
    assert.equal(factory?.name, 'example');
    assert.equal(loaded?.skills[0]?.alwaysAvailable, false);
    assert.deepEqual(loaded?.skills[0]?.skill.allowedTools, ['example']);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('rejects invalid manifests, undeclared-compatible paths, and missing files contextually', async () => {
  const root = await createRoot();
  try {
    const bundle = await createBundle(root, 'core');
    await writeFile(
      join(bundle, 'manifest.json'),
      JSON.stringify({
        name: 'core',
        description: 'x',
        tools: [{ path: '../escape.js', alwaysAvailable: false }],
        skills: [],
      }),
    );
    await assert.rejects(loadBundles(root), /manifest/i);

    await writeFile(
      join(bundle, 'manifest.json'),
      JSON.stringify({
        name: 'core',
        description: 'x',
        tools: [{ path: 'tools/missing.js', alwaysAvailable: false }],
        skills: [],
      }),
    );
    await assert.rejects(
      loadBundles(root),
      (error: unknown) =>
        error instanceof Error && error.cause instanceof Error,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('rejects incompatible exports and unresolved local tool references', async () => {
  const root = await createRoot();
  try {
    const bundle = await createBundle(root, 'core', {
      tools: [{ path: 'tools/bad.js', alwaysAvailable: false }],
    });
    await writeFile(join(bundle, 'tools', 'bad.js'), 'export default {};');
    await assert.rejects(loadBundles(root), (error: unknown) => {
      assert.ok(error instanceof Error);
      assert.match(error.message, /Unable to load tool/);
      assert.ok(error.cause instanceof Error);
      assert.match(error.cause.message, /compatible Tool/);
      return true;
    });

    await writeTool(bundle, 'available.js', 'available');
    await mkdir(join(bundle, 'skills', 'dependent'));
    await writeFile(
      join(bundle, 'skills', 'dependent', 'SKILL.md'),
      skill('dependent', ['missing']),
    );
    await writeFile(
      join(bundle, 'manifest.json'),
      JSON.stringify({
        name: 'core',
        description: 'x',
        tools: [{ path: 'tools/available.js', alwaysAvailable: false }],
        skills: [{ path: 'skills/dependent/SKILL.md', alwaysAvailable: false }],
      }),
    );
    await assert.rejects(loadBundles(root), /unavailable tool "missing"/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('rejects globally duplicated bundle, skill, and tool names', async () => {
  const scenarios = ['bundle', 'skill', 'tool'] as const;
  for (const scenario of scenarios) {
    const root = await createRoot();
    try {
      for (const directory of ['a', 'b']) {
        const bundle = await createBundle(root, directory, {
          name: scenario === 'bundle' ? 'duplicate' : directory,
          tools:
            scenario === 'tool'
              ? [{ path: 'tools/shared.js', alwaysAvailable: false }]
              : [],
          skills:
            scenario === 'skill'
              ? [{ path: 'skills/shared/SKILL.md', alwaysAvailable: false }]
              : [],
        });
        if (scenario === 'tool') await writeTool(bundle, 'shared.js', 'shared');
        if (scenario === 'skill') {
          await mkdir(join(bundle, 'skills', 'shared'));
          await writeFile(
            join(bundle, 'skills', 'shared', 'SKILL.md'),
            skill('shared'),
          );
        }
      }
      await assert.rejects(
        loadBundles(root),
        new RegExp(`Duplicate ${scenario} name`),
      );
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  }
});
