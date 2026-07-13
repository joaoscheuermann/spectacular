import assert from 'node:assert/strict';
import {
  lstat,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  symlink,
  writeFile,
} from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import test from 'node:test';

import { parseConfig } from '../src/config.js';
import { initializeWorkspace } from '../src/init.js';

test('creates a valid minimal workspace scaffold', async () => {
  const parent = await mkdtemp(join(tmpdir(), 'evolution-init-'));
  const root = join(parent, 'workspace');

  const summary = await initializeWorkspace(root);

  assert.deepEqual(summary, {
    root: resolve(root),
    configCreated: true,
    scenariosCreated: true,
  });
  assert.equal((await lstat(join(root, 'scenarios'))).isDirectory(), true);
  const text = await readFile(join(root, 'evolution.config.json'), 'utf8');
  const parsed = JSON.parse(text) as unknown;
  assert.equal(text, JSON.stringify(parsed, null, 2) + '\n');
  assert.deepEqual(parseConfig(parsed), {
    providers: [{ id: 'openai', type: 'openai', tokenEnv: 'OPENAI_API_KEY' }],
    models: [{ id: 'target-model', provider: 'openai', model: 'target-model' }],
    optimizer: { provider: 'openai', model: 'optimizer-model' },
    judge: { provider: 'openai', model: 'judge-model' },
    evals: [
      {
        id: 'correct-output',
        assertion: 'The output correctly fulfills the requested behavior.',
      },
    ],
    evolution: {
      accuracy: 0.9,
      patience: { epochs: 3 },
      epochs: 20,
      history: { limit: 30 },
    },
  });
  assert.deepEqual((await readdir(root)).sort(), [
    'evolution.config.json',
    'scenarios',
  ]);
});

test('preserves existing config and scenarios byte-for-byte', async () => {
  const root = await mkdtemp(join(tmpdir(), 'evolution-init-existing-'));
  const scenarios = join(root, 'scenarios');
  const config = join(root, 'evolution.config.json');
  const originalConfig = '{"existing":true}\r\n';
  const originalScenario = '{"existing":"scenario"}\r\n';
  await mkdir(scenarios);
  await writeFile(config, originalConfig);
  await writeFile(join(scenarios, 'existing.json'), originalScenario);

  const summary = await initializeWorkspace(root);

  assert.deepEqual(summary, {
    root: resolve(root),
    configCreated: false,
    scenariosCreated: false,
  });
  assert.equal(await readFile(config, 'utf8'), originalConfig);
  assert.equal(
    await readFile(join(scenarios, 'existing.json'), 'utf8'),
    originalScenario,
  );
});

test('rejects a config type conflict before creating scenarios', async () => {
  const root = await mkdtemp(join(tmpdir(), 'evolution-init-config-conflict-'));
  await mkdir(join(root, 'evolution.config.json'));

  await assert.rejects(initializeWorkspace(root), /regular file/);

  assert.deepEqual(await readdir(root), ['evolution.config.json']);
});

test('rejects a linked scenarios directory before creating config', async () => {
  const parent = await mkdtemp(join(tmpdir(), 'evolution-init-link-conflict-'));
  const root = join(parent, 'workspace');
  const target = join(parent, 'linked-scenarios');
  await mkdir(root);
  await mkdir(target);
  await symlink(
    target,
    join(root, 'scenarios'),
    process.platform === 'win32' ? 'junction' : 'dir',
  );

  await assert.rejects(initializeWorkspace(root), /real directory/);

  assert.deepEqual(await readdir(root), ['scenarios']);
});
