import assert from 'node:assert/strict';
import {
  access,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  writeFile,
} from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import type { CompletionFor } from '../src/completion.js';
import { runEvolution } from '../src/run.js';
import type { EvolutionConfig, Scenario } from '../src/schema.js';
import { fakeCompletion, validConfig } from './fakes.js';

const config = (): EvolutionConfig => ({
  ...validConfig(),
  models: [
    { id: 'model-a', provider: 'local', model: 'model-a' },
    { id: 'model-b', provider: 'local', model: 'model-b' },
  ],
});

const baseline: Scenario = {
  id: 'baseline-case',
  input: 'Shared baseline input',
  expected: 'Shared baseline output',
  tags: ['initialized'],
};

const workspace = async (): Promise<{
  readonly root: string;
  readonly configPath: string;
}> => {
  const root = await mkdtemp(join(tmpdir(), 'evolution-run-'));
  await mkdir(join(root, 'default'));
  await mkdir(join(root, 'scenarios'));
  await writeFile(
    join(root, 'default', 'SYSTEM_PROMPT.md'),
    'Original prompt\n',
  );
  const configPath = join(root, 'evolution.config.json');
  await writeFile(configPath, JSON.stringify(config()), 'utf8');
  return { root, configPath };
};

const completions =
  (order: string[]): CompletionFor =>
  (model) => {
    if (model.model === 'optimizer-model') {
      return fakeCompletion(undefined, async () => {
        order.push('initializer');
        return {
          scenarios: [{ ...baseline, rationale: null }],
          rationale: 'Create the shared baseline.',
        };
      });
    }
    if (model.model.startsWith('judge-')) {
      return fakeCompletion(undefined, async (_system, input) => {
        const value = JSON.parse(input) as {
          readonly originalPrompt?: string;
        };
        order.push(
          value.originalPrompt === undefined
            ? 'result-judge:' + model.model
            : 'scenario-judge:' + model.model,
        );
        return {
          passed: true,
          ambiguous: false,
          rationale: 'Clear and correct.',
        };
      });
    }
    return fakeCompletion(async (system, input) => {
      assert.equal(system, 'Original prompt\n');
      assert.equal(input, baseline.input);
      order.push('target:' + model.model);
      return baseline.expected;
    });
  };

test('initializes one shared baseline before evolving two targets and persists it as initial', async () => {
  const { root, configPath } = await workspace();
  const order: string[] = [];

  const summary = await runEvolution(
    { config: configPath, dryRun: false },
    { completionFactory: () => completions(order) },
  );

  assert.equal(order.filter((entry) => entry === 'initializer').length, 1);
  const lastScenarioJudge = Math.max(
    ...order.map((entry, index) =>
      entry.startsWith('scenario-judge:') ? index : -1,
    ),
  );
  const firstTarget = order.findIndex((entry) => entry.startsWith('target:'));
  assert.ok(lastScenarioJudge < firstTarget);
  assert.deepEqual(
    order.filter((entry) => entry.startsWith('target:')),
    ['target:model-a', 'target:model-b'],
  );
  assert.equal(summary.scenarioCount, 1);
  assert.deepEqual(
    summary.targets.map(({ acceptedScenarioCount }) => acceptedScenarioCount),
    [0, 0],
  );
  assert.deepEqual(
    JSON.parse(
      await readFile(join(root, 'scenarios', baseline.id + '.json'), 'utf8'),
    ),
    baseline,
  );
  assert.deepEqual(
    JSON.parse(
      await readFile(
        join(root, 'default', 'scenarios', baseline.id + '.json'),
        'utf8',
      ),
    ),
    baseline,
  );
});

test('runs empty-suite initialization during dry run without writing results', async () => {
  const { root, configPath } = await workspace();
  const order: string[] = [];
  const originalEntries = (await readdir(root)).sort();

  const summary = await runEvolution(
    { config: configPath, dryRun: true },
    { completionFactory: () => completions(order) },
  );

  assert.equal(summary.dryRun, true);
  assert.equal(summary.scenarioCount, 1);
  assert.equal(order.filter((entry) => entry === 'initializer').length, 1);
  assert.deepEqual((await readdir(root)).sort(), originalEntries);
  assert.deepEqual(await readdir(join(root, 'scenarios')), []);
  await assert.rejects(access(join(root, 'default', 'scenarios')));
  await assert.rejects(access(join(root, 'model-a')));
  await assert.rejects(access(join(root, 'model-b')));
});
