import assert from 'node:assert/strict';
import test from 'node:test';

import { evaluate, type Judge } from '../src/evaluate.js';
import type { ProgressEvent } from '../src/progress.js';
import { fakeCompletion, matrix } from './fakes.js';

const scenario = (id: string, evalIds: readonly string[]) => ({
  id,
  split: 'train' as const,
  input: 'Private input ' + id,
  evals: evalIds.map((evalId) => ({ id: evalId, assertion: 'Private rule' })),
});

test('calls target three times and judge once with the full assertion matrix', async () => {
  let targetCalls = 0;
  let judgeCalls = 0;
  const progress: ProgressEvent[] = [];
  const judge: Judge = {
    id: 'judge',
    completion: fakeCompletion(undefined, async (_system, input) => {
      judgeCalls += 1;
      const payload = JSON.parse(input) as {
        readonly outputs: readonly unknown[];
      };
      assert.equal(payload.outputs.length, 3);
      return { results: matrix(['a', 'b']) };
    }),
  };
  const result = await evaluate(
    fakeCompletion(async () => {
      targetCalls += 1;
      return 'Private output';
    }),
    judge,
    'Private prompt',
    [scenario('one', ['a', 'b'])],
    { targetId: 'target', progress: (event) => progress.push(event) },
  );
  assert.equal(targetCalls, 3);
  assert.equal(judgeCalls, 1);
  assert.equal(result.accuracy, 1);
  const logged = JSON.stringify(progress);
  for (const body of [
    'Private input',
    'Private output',
    'Private prompt',
    'Private rule',
  ]) {
    assert.equal(logged.includes(body), false);
  }
});

test('macro-averages scenario accuracy instead of weighting assertion counts', async () => {
  const judge: Judge = {
    id: 'judge',
    completion: fakeCompletion(undefined, async (_system, input) => {
      const payload = JSON.parse(input) as {
        readonly evals: readonly { id: string }[];
      };
      const ids = payload.evals.map(({ id }) => id);
      return { results: matrix(ids, ids.length > 1) };
    }),
  };
  const result = await evaluate(
    fakeCompletion(async () => 'output'),
    judge,
    'prompt',
    [scenario('many', ['a', 'b', 'c']), scenario('one', ['d'])],
    { targetId: 'target' },
  );
  assert.equal(result.accuracy, 0.5);
});

test('aborts on missing, duplicate, or unknown judge results', async () => {
  for (const results of [
    matrix(['a']).slice(1),
    [...matrix(['a']).slice(0, 2), matrix(['a'])[0]],
    matrix(['unknown']),
  ]) {
    await assert.rejects(
      evaluate(
        fakeCompletion(async () => 'output'),
        {
          id: 'judge',
          completion: fakeCompletion(undefined, async () => ({ results })),
        },
        'prompt',
        [scenario('case', ['a'])],
        { targetId: 'target' },
      ),
      /invalid result matrix/,
    );
  }
});
