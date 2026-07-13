import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { test, type TestContext } from 'node:test';

import { defaultOutput, generate } from '../src/index.js';
import { createProvider } from './fakes.js';

const tempRoot = async (context: TestContext): Promise<string> => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'okf-generate-'));
  context.after(() => fs.rm(root, { recursive: true, force: true }));
  return root;
};

test('generates the project bundle through the public API', async (context) => {
  const root = await tempRoot(context);
  await fs.writeFile(path.join(root, 'README.md'), '# Example\n', 'utf-8');
  const fake = createProvider();

  const result = await generate(
    { provider: fake.provider, model: 'fake' },
    root,
  );

  assert.equal(result.output, defaultOutput(root));
  assert.equal(result.index, path.join(result.output, 'index.md'));
  assert.deepEqual(result.files, ['README.md']);
  assert.equal(result.generated, 1);
  assert.equal(result.cached, 0);
  assert.equal(fake.requests.length, 3);
  const concept = await fs.readFile(
    path.join(result.output, 'README.md.md'),
    'utf-8',
  );
  assert.match(concept, /resource: "source:README\.md"/u);
  assert.match(concept, /# Subject/u);
  assert.match(
    await fs.readFile(result.index, 'utf-8'),
    /\[README\.md\]\(README\.md\.md\) - Documents README\.md\./u,
  );
});

test('uses a contained custom output and rejects other locations', async (context) => {
  const root = await tempRoot(context);
  await fs.writeFile(path.join(root, 'guide.txt'), 'Guide', 'utf-8');
  const custom = path.join(root, '.agents', 'bundles', 'custom');
  const fake = createProvider();

  const result = await generate(
    { provider: fake.provider, model: 'fake' },
    root,
    custom,
  );

  assert.equal(result.output, custom);
  await assert.rejects(
    generate(
      { provider: fake.provider, model: 'fake' },
      root,
      path.join(root, 'outside'),
    ),
    /must be inside/u,
  );
});

test('rejects an output that escapes through an existing junction', async (context) => {
  const root = await tempRoot(context);
  const outside = await tempRoot(context);
  const bundles = path.join(root, '.agents', 'bundles');
  await fs.mkdir(bundles, { recursive: true });
  await fs.symlink(outside, path.join(bundles, 'escape'), 'junction');
  const fake = createProvider();

  await assert.rejects(
    generate(
      { provider: fake.provider, model: 'fake' },
      root,
      path.join(bundles, 'escape', 'project'),
    ),
    /must be inside/u,
  );
  await assert.rejects(fs.access(path.join(outside, 'project', 'index.md')));
  assert.equal(fake.requests.length, 0);
});

test('reuses concepts whose raw source hash is unchanged', async (context) => {
  const root = await tempRoot(context);
  await fs.writeFile(path.join(root, 'guide.md'), '# Guide\n', 'utf-8');
  const fake = createProvider();
  const config = { provider: fake.provider, model: 'fake' };

  await generate(config, root);
  const result = await generate(config, root);

  assert.equal(fake.requests.length, 3);
  assert.equal(result.generated, 0);
  assert.equal(result.cached, 1);
});

test('fails when a stage does not return structured output', async (context) => {
  const root = await tempRoot(context);
  await fs.writeFile(path.join(root, 'guide.md'), '# Guide\n', 'utf-8');
  const fake = createProvider(() => undefined);

  await assert.rejects(
    generate({ provider: fake.provider, model: 'fake' }, root),
    /Classification for guide\.md did not return valid structured output/u,
  );
});

test('loads default prompts and sends JSON evidence envelopes', async (context) => {
  const root = await tempRoot(context);
  await fs.writeFile(path.join(root, 'guide.md'), '# Guide\n', 'utf-8');
  const fake = createProvider();

  await generate({ provider: fake.provider, model: 'fake' }, root);

  const systems = fake.requests.map((request) => request.messages[0]?.content);
  assert.ok(systems.every((system) => typeof system === 'string'));
  const inputs = fake.requests.map((request) => request.messages[1]?.content);
  assert.ok(
    inputs.every(
      (input) =>
        typeof input === 'string' &&
        JSON.parse(input).path === 'guide.md' &&
        !input.includes('~~~'),
    ),
  );
  await assert.rejects(
    generate(
      { provider: fake.provider, model: 'fake', promptTarget: 'missing' },
      root,
    ),
    /Cannot load OKF (?:analyze|classify|frontmatter) prompt target missing/u,
  );
  await assert.rejects(
    generate(
      { provider: fake.provider, model: 'fake', promptTarget: 'con' },
      root,
    ),
    /Invalid OKF prompt target: con/u,
  );
});

test('selects the analysis prompt for the classified file kind', async (context) => {
  const root = await tempRoot(context);
  await fs.writeFile(path.join(root, 'messages.json'), '{}', 'utf-8');
  const expected = await fs.readFile(
    path.resolve(
      'packages/okf/prompts/analyze/localization/default/SYSTEM_PROMPT.md',
    ),
    'utf-8',
  );
  const fake = createProvider((system, input, index) => {
    const evidence = JSON.parse(input) as { readonly path: string };
    if (index === 0) return { type: 'localization' };
    if (index === 1) {
      return {
        type: 'Localization',
        title: evidence.path,
        description: `Documents ${evidence.path}.`,
        tags: ['localization'],
      };
    }

    assert.equal(system, expected.trim());
    return { summary: '# Localization\n- One message catalog.' };
  });

  await generate({ provider: fake.provider, model: 'fake' }, root);

  assert.equal(fake.requests.length, 3);
});

test('skips NUL and invalid UTF-8 binary content', async (context) => {
  const root = await tempRoot(context);
  await Promise.all([
    fs.writeFile(path.join(root, 'nul.bin'), Buffer.from([65, 0, 66])),
    fs.writeFile(path.join(root, 'invalid.bin'), Buffer.from([0xff, 0xfe])),
  ]);
  const fake = createProvider();

  const result = await generate(
    { provider: fake.provider, model: 'fake' },
    root,
  );

  assert.deepEqual(result.files, []);
  assert.equal(fake.requests.length, 0);
  assert.equal(await fs.readFile(result.index, 'utf-8'), '# Project\n\n');
});
