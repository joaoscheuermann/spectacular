import assert from 'node:assert/strict';
import test from 'node:test';
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { createProgram } from '../src/cli.js';
import { loadDefaultPrompt } from '../src/run.js';

test('dispatches evolve with config path and dry run', async () => {
  let received:
    { readonly config: string; readonly dryRun: boolean } | undefined;
  const program = createProgram(
    async (options) => {
      received = options;
      return {
        dryRun: options.dryRun,
        root: 'root',
        scenarioCount: 0,
        targets: [],
      };
    },
    () => undefined,
    async () => {
      throw new Error('init should not run');
    },
  );
  await program.parseAsync([
    'node',
    'evolution',
    'evolve',
    'evolution.config.json',
    '--dry-run',
  ]);
  assert.deepEqual(received, { config: 'evolution.config.json', dryRun: true });
});

test('dispatches init with current directory by default', async () => {
  let received: string | undefined;
  const program = createProgram(
    async () => {
      throw new Error('evolve should not run');
    },
    () => undefined,
    async (directory) => {
      received = directory;
      return { root: 'root', configCreated: true, scenariosCreated: true };
    },
  );
  await program.parseAsync(['node', 'evolution', 'init']);
  assert.equal(received, '.');
});

test('loads exactly one Markdown or text default prompt', async () => {
  const root = await mkdtemp(join(tmpdir(), 'evolution-default-'));
  await mkdir(join(root, 'default'));
  await writeFile(join(root, 'default', 'SYSTEM_PROMPT.txt'), 'Text prompt');
  assert.equal(await loadDefaultPrompt(root), 'Text prompt');
  await writeFile(join(root, 'default', 'SYSTEM_PROMPT.md'), 'Markdown prompt');
  await assert.rejects(loadDefaultPrompt(root), /ambiguous/);
});
