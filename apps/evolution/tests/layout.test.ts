import assert from 'node:assert/strict';
import test from 'node:test';
import {
  access,
  mkdir,
  mkdtemp,
  readFile,
  symlink,
  writeFile,
} from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { persistLayout } from '../src/layout.js';
import type { Scenario, TargetModel } from '../src/schema.js';

const scenario = (
  id: string,
  input = 'input',
  expected = 'expected',
): Scenario => ({ id, input, expected, tags: [] });

const targetResult = (target: TargetModel, prompt: string) => ({
  target,
  prompt,
  evaluation: { score: 1, passed: 1, total: 1, results: [] },
  scenarios: [],
  acceptedScenarios: [],
  epochsRun: 1,
  stopReason: 'target-reached' as const,
});

test('writes model prompts, merged scenarios, and the default snapshot', async () => {
  const root = await mkdtemp(join(tmpdir(), 'evolution-layout-'));
  const defaultDirectory = join(root, 'default');
  await mkdir(defaultDirectory);
  await writeFile(
    join(defaultDirectory, 'SYSTEM_PROMPT.md'),
    'Original prompt\n',
  );
  await writeFile(join(root, 'evolution.config.json'), '{"owned":true}\n');
  const initial = scenario('initial');
  const accepted = scenario('accepted', 'new input', 'new expected');
  const target = {
    id: 'lfm2.5-8b-a1b',
    provider: 'local',
    model: 'model',
  };

  await persistLayout({
    root,
    initialScenarios: [initial],
    scenarios: [initial, accepted],
    targets: [targetResult(target, 'Evolved prompt')],
    dryRun: false,
  });

  assert.equal(
    await readFile(join(defaultDirectory, 'SYSTEM_PROMPT.md'), 'utf8'),
    'Original prompt\n',
  );
  assert.equal(
    await readFile(join(root, 'evolution.config.json'), 'utf8'),
    '{"owned":true}\n',
  );
  assert.deepEqual(
    JSON.parse(
      await readFile(
        join(defaultDirectory, 'scenarios', 'initial.json'),
        'utf8',
      ),
    ),
    initial,
  );
  assert.deepEqual(
    JSON.parse(
      await readFile(join(root, 'scenarios', 'accepted.json'), 'utf8'),
    ),
    accepted,
  );
  assert.equal(
    await readFile(join(root, 'lfm2.5-8b-a1b', 'SYSTEM_PROMPT.md'), 'utf8'),
    'Evolved prompt\n',
  );
});

test('dry run performs zero filesystem writes', async () => {
  const parent = await mkdtemp(join(tmpdir(), 'evolution-dry-'));
  const root = join(parent, 'not-created');
  await persistLayout({
    root,
    initialScenarios: [scenario('initial')],
    scenarios: [scenario('initial')],
    targets: [],
    dryRun: true,
  });
  await assert.rejects(access(root));
});

test('preflight rejects scenario collisions before writing model prompts', async () => {
  const root = await mkdtemp(join(tmpdir(), 'evolution-collision-'));
  await mkdir(join(root, 'scenarios'));
  await writeFile(
    join(root, 'scenarios', 'same.json'),
    JSON.stringify(scenario('same', 'owned input')),
  );
  const target = { id: 'model', provider: 'local', model: 'model' };
  await assert.rejects(
    persistLayout({
      root,
      initialScenarios: [],
      scenarios: [scenario('same', 'different input')],
      targets: [targetResult(target, 'prompt')],
      dryRun: false,
    }),
    /Scenario id collision/,
  );
  await assert.rejects(access(join(root, 'model', 'SYSTEM_PROMPT.md')));
});

test('preflight rejects model directories that are junctions', async () => {
  const root = await mkdtemp(join(tmpdir(), 'evolution-link-root-'));
  const outside = await mkdtemp(join(tmpdir(), 'evolution-link-outside-'));
  await symlink(outside, join(root, 'model'), 'junction');
  const target = { id: 'model', provider: 'local', model: 'model' };

  await assert.rejects(
    persistLayout({
      root,
      initialScenarios: [],
      scenarios: [],
      targets: [targetResult(target, 'prompt')],
      dryRun: false,
    }),
    /symbolic link or junction/,
  );
  await assert.rejects(access(join(outside, 'SYSTEM_PROMPT.md')));
});
