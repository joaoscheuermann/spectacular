import assert from 'node:assert/strict';
import test from 'node:test';
import { createHash } from 'node:crypto';
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import {
  historyFingerprint,
  loadHistory,
  type HistoryRecord,
} from '../src/history.js';
import { scenarios, validConfig } from './fakes.js';

test('fingerprints the full contract and loads only newest matching records', async () => {
  const root = await mkdtemp(join(tmpdir(), 'evolution-history-'));
  await mkdir(join(root, 'model'));
  const config = validConfig();
  const inputs = {
    originalPrompt: 'prompt',
    training: scenarios().filter(({ split }) => split === 'train'),
    globalEvals: config.evals,
    accuracy: config.evolution.accuracy,
    target: config.models[0]!,
    judge: config.judge,
  };
  const fingerprint = historyFingerprint(inputs);
  const expected = createHash('sha256')
    .update(
      JSON.stringify({
        evaluationMode: 'per-eval-sample-v1',
        ...inputs,
      }),
    )
    .digest('hex');
  assert.equal(fingerprint, expected);
  const entry = (fingerprintValue: string, prompt: string): HistoryRecord => ({
    fingerprint: fingerprintValue,
    attemptedPrompt: prompt,
    strategy: prompt,
    optimizerMode: 'normal',
    trainingAccuracy: 0,
    failedEvals: [],
    disposition: 'not-improved',
    terminalStatus: null,
  });
  await writeFile(
    join(root, 'model', 'evolution.history.jsonl'),
    [
      entry('stale', 'stale'),
      entry(fingerprint, 'one'),
      entry(fingerprint, 'two'),
    ]
      .map((value) => JSON.stringify(value))
      .join('\n') + '\n',
  );
  const loaded = await loadHistory(root, 'model', fingerprint, 1);
  assert.deepEqual(
    loaded.map(({ attemptedPrompt }) => attemptedPrompt),
    ['two'],
  );
  assert.notEqual(
    fingerprint,
    historyFingerprint({
      originalPrompt: 'changed',
      training: scenarios().filter(({ split }) => split === 'train'),
      globalEvals: config.evals,
      accuracy: config.evolution.accuracy,
      target: config.models[0]!,
      judge: config.judge,
    }),
  );
});
