import assert from 'node:assert/strict';
import test from 'node:test';

import { parseConfig } from '../src/config.js';
import { judgmentSchema } from '../src/schema.js';
import { validConfig } from './fakes.js';

test('accepts dotted model folder ids and validates references', () => {
  const config = parseConfig(validConfig());
  assert.equal(config.models[0]?.id, 'lfm2.5-8b-a1b');
});

test('rejects unsafe and reserved model folder ids', () => {
  for (const id of [
    '../escape',
    '.',
    '..',
    'default',
    'scenarios',
    'bad.',
    'con',
    'con.txt',
    'prn.md',
    'aux',
    'nul.json',
    'com1',
    'com9.prompt',
    'lpt1',
    'lpt9.txt',
  ]) {
    const config = validConfig();
    assert.throws(() =>
      parseConfig({
        ...config,
        models: [{ ...config.models[0], id }],
      }),
    );
  }
});

test('rejects duplicate ids, duplicate judges, and unknown providers', () => {
  const config = validConfig();
  assert.throws(() =>
    parseConfig({
      ...config,
      providers: [...config.providers, config.providers[0]],
    }),
  );
  assert.throws(() =>
    parseConfig({
      ...config,
      models: [...config.models, config.models[0]],
    }),
  );
  assert.throws(() =>
    parseConfig({
      ...config,
      judges: [config.judges[0], config.judges[0]],
    }),
  );
  assert.throws(
    () =>
      parseConfig({
        ...config,
        optimizer: { provider: 'missing', model: 'optimizer-model' },
      }),
    /Unknown provider reference/,
  );
});

test('requires the exact structured judgment contract', () => {
  assert.deepEqual(
    judgmentSchema.parse({
      passed: true,
      ambiguous: false,
      rationale: 'Clear.',
    }),
    { passed: true, ambiguous: false, rationale: 'Clear.' },
  );
  assert.throws(() =>
    judgmentSchema.parse({
      pass: true,
      ambiguous: false,
      rationale: 'Clear.',
    }),
  );
});
