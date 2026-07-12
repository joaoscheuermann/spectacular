import assert from 'node:assert/strict';
import test from 'node:test';
import { mkdir, mkdtemp, readFile, readdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, relative } from 'node:path';

import { createProgram } from '../src/cli.js';
import type { CompletionFor } from '../src/completion.js';
import { loadDefaultPrompt, runEvolution } from '../src/run.js';
import { fakeCompletion, validConfig } from './fakes.js';

test('parses exactly evolve config-path with optional dry run', async () => {
  let received:
    | { readonly config: string; readonly dryRun: boolean }
    | undefined;
  let emitted = false;
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
    () => {
      emitted = true;
    },
  );
  await program.parseAsync([
    'node',
    'evolve',
    'relative/evolution.config.json',
    '--dry-run',
  ]);
  assert.deepEqual(received, {
    config: 'relative/evolution.config.json',
    dryRun: true,
  });
  assert.equal(emitted, true);
  assert.equal(program.name(), 'evolve');
  assert.equal(
    program.options.some(({ long }) => long === '--config'),
    false,
  );
  assert.equal(
    program.options.some(({ long }) => long === '--output'),
    false,
  );
});

test('loads exactly one Markdown or text default prompt', async () => {
  const root = await mkdtemp(join(tmpdir(), 'evolution-default-'));
  await mkdir(join(root, 'default'));
  await writeFile(join(root, 'default', 'SYSTEM_PROMPT.txt'), 'Text prompt');
  assert.equal(await loadDefaultPrompt(root), 'Text prompt');
  await writeFile(join(root, 'default', 'SYSTEM_PROMPT.md'), 'Markdown prompt');
  await assert.rejects(loadDefaultPrompt(root), /ambiguous/);
  const missing = await mkdtemp(join(tmpdir(), 'evolution-missing-'));
  await assert.rejects(loadDefaultPrompt(missing), /missing/);
});

const snapshot = async (
  root: string,
  directory = root,
): Promise<readonly (readonly [string, string])[]> => {
  const entries = await readdir(directory, { withFileTypes: true });
  const nested = await Promise.all(
    entries.map(
      async (entry): Promise<readonly (readonly [string, string])[]> => {
        const path = join(directory, entry.name);
        return entry.isDirectory()
          ? snapshot(root, path)
          : [[relative(root, path), await readFile(path, 'utf8')]];
      },
    ),
  );
  return nested.flat().sort(([left], [right]) => left.localeCompare(right));
};

test('runs provider calls in dry mode without changing the workspace', async () => {
  const root = await mkdtemp(join(tmpdir(), 'evolution-run-dry-'));
  await mkdir(join(root, 'default'));
  await mkdir(join(root, 'scenarios'));
  const config = validConfig();
  const configPath = join(root, 'evolution.config.json');
  await writeFile(configPath, JSON.stringify(config, null, 2) + '\n');
  await writeFile(
    join(root, 'default', 'SYSTEM_PROMPT.md'),
    'Private original prompt\n',
  );
  await writeFile(
    join(root, 'scenarios', 'case.json'),
    JSON.stringify({
      id: 'case',
      input: 'Private input',
      expected: 'Private expected',
      tags: [],
    }),
  );
  const before = await snapshot(root);
  let targetCalls = 0;
  let judgeCalls = 0;
  const completeFor: CompletionFor = (model) =>
    model.model.startsWith('judge-')
      ? fakeCompletion(undefined, async () => {
          judgeCalls += 1;
          return {
            passed: true,
            ambiguous: false,
            rationale: 'Private rationale',
          };
        })
      : fakeCompletion(async () => {
          targetCalls += 1;
          return 'Private output';
        });

  const result = await runEvolution(
    { config: relative(process.cwd(), configPath), dryRun: true },
    { completionFactory: () => completeFor },
  );

  assert.equal(targetCalls, 1);
  assert.equal(judgeCalls, 2);
  assert.equal(result.root, root);
  assert.deepEqual(await snapshot(root), before);
  const serialized = JSON.stringify(result);
  for (const body of [
    'Private original prompt',
    'Private input',
    'Private expected',
    'Private rationale',
    'Private output',
  ]) {
    assert.equal(serialized.includes(body), false);
  }
});
