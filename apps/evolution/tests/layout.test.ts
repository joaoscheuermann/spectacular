import assert from 'node:assert/strict';
import test from 'node:test';
import {
  access,
  mkdtemp,
  readFile,
  symlink,
  writeFile,
} from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { persistLayout } from '../src/layout.js';
import type { TargetResult } from '../src/evolve.js';

const result = (id: string, approved: boolean): TargetResult => ({
  target: { id, provider: 'local', model: id },
  prompt: 'New prompt',
  trainingEvaluation: { accuracy: 1, results: [], failures: [] },
  validationEvaluation: {
    accuracy: approved ? 1 : 0,
    results: [],
    failures: [],
  },
  trainingAccuracy: 1,
  validationAccuracy: approved ? 1 : 0,
  approved,
  refactored: false,
  epochsRun: 1,
  stopReason: approved ? 'approved' : 'validation-failed',
  history: [],
});

test('writes only approved prompts and preserves failed target prompts', async () => {
  const root = await mkdtemp(join(tmpdir(), 'evolution-layout-'));
  await writeFile(join(root, 'owned.txt'), 'owned');
  await persistLayout({
    root,
    targets: [result('approved', true), result('failed', false)],
    dryRun: false,
  });
  assert.equal(
    await readFile(join(root, 'approved', 'SYSTEM_PROMPT.md'), 'utf8'),
    'New prompt\n',
  );
  await assert.rejects(access(join(root, 'failed', 'SYSTEM_PROMPT.md')));
  assert.equal(await readFile(join(root, 'owned.txt'), 'utf8'), 'owned');
});

test('dry run performs zero filesystem writes', async () => {
  const parent = await mkdtemp(join(tmpdir(), 'evolution-dry-'));
  const root = join(parent, 'not-created');
  await persistLayout({ root, targets: [result('model', true)], dryRun: true });
  await assert.rejects(access(root));
});

test('rejects approved model directories that are symlinks', async () => {
  const root = await mkdtemp(join(tmpdir(), 'evolution-link-root-'));
  const outside = await mkdtemp(join(tmpdir(), 'evolution-link-outside-'));
  await symlink(outside, join(root, 'model'), 'junction');
  await assert.rejects(
    persistLayout({ root, targets: [result('model', true)], dryRun: false }),
    /symbolic link or junction/,
  );
  await assert.rejects(access(join(outside, 'SYSTEM_PROMPT.md')));
});
