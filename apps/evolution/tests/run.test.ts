import assert from 'node:assert/strict';
import test from 'node:test';
import {
  access,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  symlink,
  writeFile,
} from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import type { CompletionFor } from '../src/completion.js';
import { runEvolution } from '../src/run.js';
import { compressionSystemPrompt } from '../src/prompts.js';
import { fakeCompletion, judgment, validConfig } from './fakes.js';

const workspace = async () => {
  const root = await mkdtemp(join(tmpdir(), 'evolution-run-'));
  await mkdir(join(root, 'default'));
  await mkdir(join(root, 'scenarios'));
  const configPath = join(root, 'evolution.config.json');
  await writeFile(configPath, JSON.stringify(validConfig()), 'utf8');
  await writeFile(join(root, 'default', 'SYSTEM_PROMPT.md'), 'x'.repeat(100));
  await writeFile(
    join(root, 'scenarios', 'train.json'),
    JSON.stringify({ id: 'train', split: 'train', input: 'train', evals: [] }),
  );
  await writeFile(
    join(root, 'scenarios', 'validation.json'),
    JSON.stringify({
      id: 'validation',
      split: 'validation',
      input: 'validation',
      evals: [],
    }),
  );
  return { root, configPath };
};

const completions =
  (optimizerInputs: string[]): CompletionFor =>
  (model) => {
    if (model.model === 'optimizer-model') {
      return fakeCompletion(undefined, async (system, input) => {
        optimizerInputs.push(input);
        assert.equal(system, compressionSystemPrompt);
        return { prompt: 'y'.repeat(75), strategy: 'compress' };
      });
    }
    if (model.model === 'judge-model') {
      return fakeCompletion(undefined, async () => judgment());
    }
    return fakeCompletion(async () => 'correct');
  };

test('writes approved prompt and append-only training history without scenario writes', async () => {
  const { root, configPath } = await workspace();
  const optimizerInputs: string[] = [];
  const summary = await runEvolution(
    { config: configPath, dryRun: false },
    { completionFactory: () => completions(optimizerInputs) },
  );
  assert.equal(summary.targets[0]?.approved, true);
  assert.equal(
    await readFile(join(root, 'model', 'SYSTEM_PROMPT.md'), 'utf8'),
    'y'.repeat(75) + '\n',
  );
  const history = await readFile(
    join(root, 'model', 'evolution.history.jsonl'),
    'utf8',
  );
  assert.equal(history.trim().split('\n').length, 1);
  assert.equal(history.includes('validation'), false);
  assert.deepEqual((await readdir(join(root, 'scenarios'))).sort(), [
    'train.json',
    'validation.json',
  ]);
});

test('dry run reads providers but performs no prompt or history writes', async () => {
  const { root, configPath } = await workspace();
  await runEvolution(
    { config: configPath, dryRun: true },
    { completionFactory: () => completions([]) },
  );
  await assert.rejects(access(join(root, 'model')));
});

test('rejects an incomplete suite before constructing completions', async () => {
  const { root, configPath } = await workspace();
  await writeFile(
    join(root, 'scenarios', 'validation.json'),
    JSON.stringify({
      id: 'validation',
      split: 'train',
      input: 'other',
      evals: [],
    }),
  );
  let factories = 0;
  await assert.rejects(
    runEvolution(
      { config: configPath, dryRun: true },
      {
        completionFactory: () => {
          factories += 1;
          return completions([]);
        },
      },
    ),
    /validation scenario/,
  );
  assert.equal(factories, 0);
});

test('preflights an approved prompt path before appending terminal history', async () => {
  const { root, configPath } = await workspace();
  const model = join(root, 'model');
  const outside = join(root, 'outside-prompt.md');
  await mkdir(model);
  await writeFile(outside, 'outside-owned\n');
  await symlink(outside, join(model, 'SYSTEM_PROMPT.md'));

  await assert.rejects(
    runEvolution(
      { config: configPath, dryRun: false },
      { completionFactory: () => completions([]) },
    ),
    /targets failed/,
  );

  await assert.rejects(access(join(model, 'evolution.history.jsonl')));
  assert.equal(await readFile(outside, 'utf8'), 'outside-owned\n');
});

test('isolates malformed target history and persists unaffected targets', async () => {
  const { root, configPath } = await workspace();
  const config = validConfig();
  await writeFile(
    configPath,
    JSON.stringify({
      ...config,
      models: [
        { id: 'damaged', provider: 'local', model: 'damaged-model' },
        { id: 'healthy', provider: 'local', model: 'healthy-model' },
      ],
    }),
  );
  await mkdir(join(root, 'damaged'));
  await writeFile(
    join(root, 'damaged', 'evolution.history.jsonl'),
    '{"malformed":true}\n',
  );
  const targetModels: string[] = [];
  const completeFor: CompletionFor = (model) => {
    if (model.model === 'optimizer-model') {
      return fakeCompletion(undefined, async () => ({
        prompt: 'y'.repeat(75),
        strategy: 'compress',
      }));
    }
    if (model.model === 'judge-model') {
      return fakeCompletion(undefined, async () => judgment());
    }
    return fakeCompletion(async () => {
      targetModels.push(model.model);
      return 'correct';
    });
  };

  await assert.rejects(
    runEvolution(
      { config: configPath, dryRun: false },
      { completionFactory: () => completeFor },
    ),
    /targets failed/,
  );

  assert.equal(targetModels.includes('damaged-model'), false);
  assert.equal(targetModels.includes('healthy-model'), true);
  assert.equal(
    await readFile(join(root, 'healthy', 'SYSTEM_PROMPT.md'), 'utf8'),
    'y'.repeat(75) + '\n',
  );
  assert.match(
    await readFile(join(root, 'healthy', 'evolution.history.jsonl'), 'utf8'),
    /"terminalStatus":"approved"/,
  );
  await assert.rejects(access(join(root, 'damaged', 'SYSTEM_PROMPT.md')));
});
