import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import test from 'node:test';

import { loadConfig, parseConfig } from '../src/config.js';
import { loadScenarios, prepareScenarios } from '../src/scenarios.js';
import { scenarioSchema } from '../src/schema.js';
import { validConfig } from './fakes.js';

test('defaults nested history and concurrency settings', () => {
  const config = validConfig();
  const evolution = {
    accuracy: config.evolution.accuracy,
    patience: config.evolution.patience,
    epochs: config.evolution.epochs,
  };
  const parsed = parseConfig({ ...config, evolution });
  assert.equal(parsed.evolution.history.limit, 20);
  assert.deepEqual(parsed.evolution.concurrency, {
    scenarios: 1,
    judgments: 1,
  });
  assert.equal(parsed.judge.model, 'judge-model');
});

test('accepts explicit and partial concurrency settings', () => {
  const config = validConfig();
  assert.deepEqual(
    parseConfig({
      ...config,
      evolution: {
        ...config.evolution,
        concurrency: { scenarios: 4, judgments: 7 },
      },
    }).evolution.concurrency,
    { scenarios: 4, judgments: 7 },
  );
  assert.deepEqual(
    parseConfig({
      ...config,
      evolution: {
        ...config.evolution,
        concurrency: { scenarios: 3 },
      },
    }).evolution.concurrency,
    { scenarios: 3, judgments: 1 },
  );
  assert.deepEqual(
    parseConfig({
      ...config,
      evolution: {
        ...config.evolution,
        concurrency: { judgments: 5 },
      },
    }).evolution.concurrency,
    { scenarios: 1, judgments: 5 },
  );
});

test('rejects unknown and non-positive-integer concurrency settings', () => {
  const config = validConfig();
  for (const concurrency of [
    { scenarios: 1, judgments: 1, extra: 1 },
    { scenarios: 0, judgments: 1 },
    { scenarios: -1, judgments: 1 },
    { scenarios: 1.5, judgments: 1 },
    { scenarios: 1, judgments: 0 },
    { scenarios: 1, judgments: -1 },
    { scenarios: 1, judgments: 1.5 },
  ]) {
    assert.throws(() =>
      parseConfig({
        ...config,
        evolution: { ...config.evolution, concurrency },
      }),
    );
  }
});

test('strictly rejects legacy fields and optimizer temperature', () => {
  const config = validConfig();
  for (const value of [
    { ...config, judges: [config.judge] },
    {
      ...config,
      evolution: { ...config.evolution, targetAccuracy: 1 },
    },
    { ...config, optimizer: { ...config.optimizer, temperature: 0.4 } },
    {
      ...config,
      evolution: { ...config.evolution, plateauPatience: 2 },
    },
    { ...config, evolution: { ...config.evolution, maxEpochs: 2 } },
    { ...config, evolution: { ...config.evolution, historyLimit: 2 } },
  ]) {
    assert.throws(() => parseConfig(value));
  }
  for (const legacy of [
    { id: 'case', split: 'train', input: 'x', evals: [], expected: 'x' },
    { id: 'case', split: 'train', input: 'x', evals: [], tags: [] },
  ]) {
    assert.throws(() => scenarioSchema.parse(legacy));
  }
});

test('merges global evals first and requires both suite splits', () => {
  const globals = [{ id: 'global', assertion: 'global assertion' }];
  const suite = prepareScenarios(
    [
      {
        id: 'train',
        split: 'train',
        input: 'train input',
        evals: [{ id: 'local', assertion: 'local assertion' }],
      },
      {
        id: 'validation',
        split: 'validation',
        input: 'validation input',
        evals: [],
      },
    ],
    globals,
  );
  assert.deepEqual(
    suite[0]?.evals.map(({ id }) => id),
    ['global', 'local'],
  );
  assert.throws(
    () => prepareScenarios(suite.slice(0, 1), globals),
    /validation/,
  );
  assert.throws(
    () =>
      prepareScenarios(
        [
          ...suite,
          {
            id: 'duplicate',
            split: 'train',
            input: 'different',
            evals: [{ id: 'global', assertion: 'collision' }],
          },
        ],
        globals,
      ),
    /duplicate effective eval ids/,
  );
});

test('loads the tracked OKF config and six curated scenarios', async () => {
  const root = join(
    process.cwd(),
    'packages',
    'okf',
    'prompts',
    'analyze',
    'source_code',
  );
  const path = join(root, 'evolution.config.json');
  const tracked = JSON.parse(await readFile(path, 'utf8')) as {
    readonly evolution?: { readonly concurrency?: unknown };
  };
  assert.deepEqual(tracked.evolution?.concurrency, {
    scenarios: 1,
    judgments: 1,
  });
  const config = await loadConfig(path);
  const scenarios = await loadScenarios(join(root, 'scenarios'), config.evals);
  assert.equal(scenarios.length, 6);
  assert.equal(scenarios.filter(({ split }) => split === 'train').length, 4);
  assert.equal(
    scenarios.filter(({ split }) => split === 'validation').length,
    2,
  );
  assert.ok(scenarios.every(({ evals }) => evals.length === 4));
});
