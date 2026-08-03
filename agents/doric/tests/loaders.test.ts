import assert from 'node:assert/strict';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { basename, join, relative, resolve } from 'node:path';
import test from 'node:test';

import { bundles } from '../src/lib/loaders/index.js';

type ToolDefinition = {
  name: string;
  description?: string;
  inputSchema: Record<string, unknown>;
  outputSchema?: Record<string, unknown>;
  strict?: boolean;
};

type Fixture = {
  manifest?: string;
  skills?: Record<string, string>;
  tools?: Record<string, ToolDefinition>;
};

const skill = (name: string, allowedTools: string[] = []): string => `---
name: ${name}
description: ${name} description
allowed-tools: [${allowedTools.join(', ')}]
---

# ${name}

Follow the ${name} procedure.
`;

const tool = (
  name: string,
  description = `${name} description`,
): ToolDefinition => ({
  name,
  description,
  inputSchema: {
    type: 'object',
    properties: {},
    additionalProperties: false,
  },
  strict: true,
});

const createRoot = async (): Promise<string> => {
  const root = await mkdtemp(join(tmpdir(), 'doric-bundles-'));
  await writeFile(join(root, 'package.json'), '{"type":"module"}\n');
  return root;
};

const createBundle = async (
  root: string,
  name: string,
  fixture: Fixture = {},
): Promise<string> => {
  const bundle = join(root, name);
  const skills = join(bundle, 'skills');
  const tools = join(bundle, 'tools');

  await Promise.all([
    mkdir(skills, { recursive: true }),
    mkdir(tools, { recursive: true }),
  ]);
  await writeFile(join(bundle, 'manifest.json'), fixture.manifest ?? '{}\n');

  await Promise.all([
    ...Object.entries(fixture.skills ?? {}).map(async ([entry, content]) => {
      const directory = join(skills, entry);
      await mkdir(directory);
      await writeFile(join(directory, 'SKILL.md'), content);
    }),
    ...Object.entries(fixture.tools ?? {}).map(([file, definition]) =>
      writeFile(
        join(tools, `${file}.js`),
        `export default { definition: ${JSON.stringify(definition)} };\n`,
      ),
    ),
  ]);

  return bundle;
};

const removeRoot = async (root: string): Promise<void> => {
  await rm(root, { recursive: true, force: true });
};

test('loads every real bundle from relative and absolute root paths', async () => {
  const absolutePath = resolve('agents/doric/bundles');
  const relativePath = relative(process.cwd(), absolutePath);

  const fromRelativePath = await bundles(relativePath);
  const fromAbsolutePath = await bundles(absolutePath);

  assert.deepEqual(fromRelativePath, fromAbsolutePath);
  assert.equal(fromAbsolutePath.skills.length, 50);
  assert.equal(fromAbsolutePath.tools.length, 27);
  assert.equal(fromAbsolutePath.skills[0]?.name, 'append-only-file-update');
  assert.equal(fromAbsolutePath.skills.at(-1)?.name, 'test-failure-diagnosis');
  assert.deepEqual(
    fromAbsolutePath.tools.slice(0, 6).map(({ name }) => name),
    ['append', 'diff', 'git_history', 'package_info', 'read', 'write'],
  );
  assert.ok(
    fromAbsolutePath.tools.every(
      (descriptor) =>
        Object.hasOwn(descriptor, 'outputSchema') &&
        Object.keys(descriptor.outputSchema).length === 0 &&
        !Object.hasOwn(descriptor, 'execute'),
    ),
  );
});

test('loads recognized child bundles in lexical order and ignores unrelated entries', async () => {
  const root = await createRoot();

  try {
    await createBundle(root, 'z-bundle', {
      skills: { zeta: skill('zeta') },
      tools: { zeta: tool('zeta') },
    });
    await createBundle(root, 'a-bundle', {
      skills: { alpha: skill('alpha') },
      tools: { alpha: tool('alpha') },
    });
    await mkdir(join(root, 'ignored', 'skills'), { recursive: true });
    await writeFile(join(root, 'ignored', 'manifest.txt'), 'not a bundle\n');
    await writeFile(join(root, 'ordinary-file'), 'ignored\n');

    const loaded = await bundles(root);

    assert.deepEqual(
      loaded.skills.map(({ name }) => name),
      ['alpha', 'zeta'],
    );
    assert.deepEqual(
      loaded.tools.map(({ name }) => name),
      ['alpha', 'zeta'],
    );
  } finally {
    await removeRoot(root);
  }
});

test('returns empty arrays when the root has no recognized bundles', async () => {
  const root = await createRoot();

  try {
    await mkdir(join(root, 'unrelated'));

    assert.deepEqual(await bundles(root), { skills: [], tools: [] });
  } finally {
    await removeRoot(root);
  }
});

test('deduplicates structurally equal tools across bundles', async () => {
  const root = await createRoot();

  try {
    await createBundle(root, 'a-bundle', {
      skills: { first: skill('first', ['shared']) },
      tools: { shared: tool('shared') },
    });
    await createBundle(root, 'b-bundle', {
      skills: { second: skill('second', ['shared']) },
      tools: { shared: tool('shared') },
    });

    const loaded = await bundles(root);

    assert.deepEqual(
      loaded.tools.map(({ name }) => name),
      ['shared'],
    );
    assert.deepEqual(
      loaded.skills.map(({ name }) => name),
      ['first', 'second'],
    );
  } finally {
    await removeRoot(root);
  }
});

test('rejects conflicting tool descriptors with both bundle contexts', async () => {
  const root = await createRoot();

  try {
    await createBundle(root, 'a-bundle', {
      tools: { shared: tool('shared', 'first definition') },
    });
    await createBundle(root, 'b-bundle', {
      tools: { shared: tool('shared', 'second definition') },
    });

    await assert.rejects(bundles(root), (received: unknown) => {
      assert.ok(received instanceof Error);
      assert.match(received.message, /shared/);
      assert.match(received.message, /a-bundle/);
      assert.match(received.message, /b-bundle/);
      assert.match(received.message, /tools[\\/]shared\.js/i);
      return true;
    });
  } finally {
    await removeRoot(root);
  }
});

test('rejects duplicate skill names across bundles with both file contexts', async () => {
  const root = await createRoot();

  try {
    await createBundle(root, 'a-bundle', {
      skills: { first: skill('duplicate') },
    });
    await createBundle(root, 'b-bundle', {
      skills: { second: skill('duplicate') },
    });

    await assert.rejects(bundles(root), (received: unknown) => {
      assert.ok(received instanceof Error);
      assert.match(received.message, /duplicate.*skill|skill.*duplicate/i);
      assert.match(received.message, /a-bundle/);
      assert.match(received.message, /skills[\\/]first[\\/]SKILL\.md/i);
      assert.match(received.message, /b-bundle/);
      assert.match(received.message, /skills[\\/]second[\\/]SKILL\.md/i);
      return true;
    });
  } finally {
    await removeRoot(root);
  }
});

test('validates allowed tools within the declaring bundle', async () => {
  const root = await createRoot();

  try {
    await createBundle(root, 'a-provider', {
      tools: { shared: tool('shared') },
    });
    await createBundle(root, 'b-consumer', {
      skills: { dependent: skill('dependent', ['shared']) },
    });

    await assert.rejects(bundles(root), (received: unknown) => {
      assert.ok(received instanceof Error);
      assert.match(received.message, /dependent/);
      assert.match(received.message, /shared/);
      assert.match(received.message, /b-consumer/);
      assert.match(received.message, /skills[\\/]dependent[\\/]SKILL\.md/i);
      return true;
    });
  } finally {
    await removeRoot(root);
  }
});

test('rejects a malformed manifest with bundle context and its original cause', async () => {
  const root = await createRoot();
  const bundle = await createBundle(root, 'broken-bundle', {
    manifest: '{',
  });

  try {
    await assert.rejects(bundles(root), (received: unknown) => {
      assert.ok(received instanceof Error);
      assert.match(received.message, /manifest\.json/i);
      assert.match(received.message, new RegExp(basename(bundle)));
      assert.ok(received.cause instanceof Error);
      return true;
    });
  } finally {
    await removeRoot(root);
  }
});

test('rejects a non-object manifest with bundle and file context', async () => {
  const root = await createRoot();

  try {
    await createBundle(root, 'array-manifest', { manifest: '[]\n' });

    await assert.rejects(bundles(root), (received: unknown) => {
      assert.ok(received instanceof Error);
      assert.match(received.message, /manifest\.json/i);
      assert.match(received.message, /array-manifest/);
      assert.match(received.message, /object/i);
      return true;
    });
  } finally {
    await removeRoot(root);
  }
});

test('preserves the original cause when a recognized bundle skill cannot be parsed', async () => {
  const root = await createRoot();
  const bundle = await createBundle(root, 'broken-bundle', {
    skills: {
      broken: `---
name: [
---

# Broken
`,
    },
  });

  try {
    await assert.rejects(bundles(root), (received: unknown) => {
      assert.ok(received instanceof Error);
      assert.match(received.message, /skills[\\/]broken[\\/]SKILL\.md/i);
      assert.match(received.message, new RegExp(basename(bundle)));
      assert.ok(received.cause instanceof Error);
      return true;
    });
  } finally {
    await removeRoot(root);
  }
});
