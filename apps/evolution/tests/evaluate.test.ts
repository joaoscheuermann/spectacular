import assert from 'node:assert/strict';
import test from 'node:test';

import { evaluate, type Judge } from '../src/evaluate.js';
import { judgeInput } from '../src/prompts.js';
import type { ProgressEvent } from '../src/progress.js';
import { deferred, fakeCompletion, judgment } from './fakes.js';

const turn = (): Promise<void> =>
  new Promise((resolve) => setImmediate(resolve));

const scenario = (id: string, evalIds: readonly string[]) => ({
  id,
  split: 'train' as const,
  input: 'Private input ' + id,
  evals: evalIds.map((evalId) => ({
    id: evalId,
    assertion: 'Private rule ' + evalId,
  })),
});

test('calls the judge once for each eval and sample pair', async () => {
  let targetCalls = 0;
  let judgeCalls = 0;
  const judgeInputs: string[] = [];
  const progress: ProgressEvent[] = [];
  const judge: Judge = {
    id: 'judge',
    completion: fakeCompletion(undefined, async (_system, input) => {
      judgeCalls += 1;
      judgeInputs.push(input);
      return judgment();
    }),
  };
  const result = await evaluate(
    fakeCompletion(async () => {
      targetCalls += 1;
      return 'Private output';
    }),
    judge,
    'Private prompt',
    [scenario('one', ['a', 'b', 'c', 'd'])],
    { targetId: 'target', progress: (event) => progress.push(event) },
  );
  assert.equal(targetCalls, 3);
  assert.equal(judgeCalls, 12);
  const expectedPairs = ['a', 'b', 'c', 'd'].flatMap((evalId) =>
    [0, 1, 2].map((sampleIndex) => ({ evalId, sampleIndex })),
  );
  assert.deepEqual(
    judgeInputs,
    ['a', 'b', 'c', 'd'].flatMap((evalId) =>
      [0, 1, 2].map((sampleIndex) =>
        judgeInput({
          scenarioInput: 'Private input one',
          assertion: {
            id: evalId,
            assertion: 'Private rule ' + evalId,
          },
          sampleIndex,
          modelOutput: 'Private output',
        }),
      ),
    ),
  );
  assert.equal(judgeInputs.every((input) => input.startsWith('# ')), true);
  assert.throws(() => JSON.parse(judgeInputs[0] ?? ''), SyntaxError);
  assert.deepEqual(
    result.results[0]?.verdicts.map(({ evalId, sampleIndex }) => ({
      evalId,
      sampleIndex,
    })),
    expectedPairs,
  );
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
      return judgment(!input.includes('Private rule d'));
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

test('rejects malformed single verdicts', async () => {
  for (const verdict of [
    { passed: true },
    { reasoning: '', passed: true },
    { reasoning: 'Clear.', passed: 'yes' },
    { reasoning: 'Clear.', passed: true, evalId: 'a' },
  ]) {
    await assert.rejects(
      evaluate(
        fakeCompletion(async () => 'output'),
        {
          id: 'judge',
          completion: fakeCompletion(undefined, async () => verdict),
        },
        'prompt',
        [scenario('case', ['a'])],
        { targetId: 'target' },
      ),
      { name: 'ZodError' },
    );
  }
});

test('defaults to end-to-end sequential scenario evaluation', async () => {
  const firstSamples = deferred<void>();
  const firstJudgment = deferred<void>();
  const events: string[] = [];
  let firstSampleStarts = 0;
  let firstJudgments = 0;
  const evaluation = evaluate(
    fakeCompletion(async (_system, input) => {
      events.push('target:' + input);
      if (input.endsWith('one')) {
        firstSampleStarts += 1;
        await firstSamples.promise;
      }
      return 'output';
    }),
    {
      id: 'judge',
      completion: fakeCompletion(undefined, async (_system, input) => {
        const scenarioInput = input.includes('Private input one')
          ? 'Private input one'
          : 'Private input two';
        events.push('judge-start:' + scenarioInput);
        if (scenarioInput.endsWith('one') && firstJudgments === 0) {
          await firstJudgment.promise;
        }
        if (scenarioInput.endsWith('one')) firstJudgments += 1;
        events.push('judge-end:' + scenarioInput);
        return judgment();
      }),
    },
    'prompt',
    [scenario('one', ['a']), scenario('two', ['a'])],
    { targetId: 'target' },
  );

  await turn();
  assert.equal(firstSampleStarts, 3);
  assert.equal(events.filter((event) => event.endsWith('two')).length, 0);
  assert.equal(
    events.filter((event) => event.startsWith('judge-start')).length,
    0,
  );

  firstSamples.resolve(undefined);
  await turn();
  assert.equal(
    events.filter((event) => event === 'judge-start:Private input one').length,
    1,
  );

  firstJudgment.resolve(undefined);
  await evaluation;
  assert.equal(firstJudgments, 3);
  assert.ok(
    events.lastIndexOf('judge-end:Private input one') <
      events.indexOf('target:Private input two'),
  );
});

test('respects scenario concurrency and refills a freed slot', async () => {
  const gates = new Map([
    ['Private input one', deferred<void>()],
    ['Private input two', deferred<void>()],
  ]);
  const targetInputs: string[] = [];
  const evaluation = evaluate(
    fakeCompletion(async (_system, input) => {
      targetInputs.push(input);
      return 'output';
    }),
    {
      id: 'judge',
      completion: fakeCompletion(undefined, async (_system, input) => {
        const scenarioInput = [...gates.keys()].find((value) =>
          input.includes(value),
        );
        await (scenarioInput === undefined
          ? Promise.resolve()
          : gates.get(scenarioInput)?.promise);
        return judgment();
      }),
    },
    'prompt',
    [scenario('one', ['a']), scenario('two', ['a']), scenario('three', ['a'])],
    {
      targetId: 'target',
      concurrency: { scenarios: 2, judgments: 6 },
    },
  );

  await turn();
  assert.equal(targetInputs.length, 6);
  assert.equal(targetInputs.includes('Private input three'), false);

  gates.get('Private input two')?.resolve(undefined);
  await turn();
  assert.equal(
    targetInputs.filter((input) => input === 'Private input three').length,
    3,
  );

  gates.get('Private input one')?.resolve(undefined);
  await evaluation;
});

test('applies one global sliding judgment cap across active scenarios', async () => {
  const gates: ReturnType<typeof deferred<void>>[] = [];
  let active = 0;
  let maximum = 0;
  let started = 0;
  let blocking = true;
  const evaluation = evaluate(
    fakeCompletion(async () => 'output'),
    {
      id: 'judge',
      completion: fakeCompletion(undefined, async () => {
        started += 1;
        active += 1;
        maximum = Math.max(maximum, active);
        const gate = deferred<void>();
        gates.push(gate);
        if (blocking) await gate.promise;
        active -= 1;
        return judgment();
      }),
    },
    'prompt',
    [scenario('one', ['a']), scenario('two', ['a'])],
    {
      targetId: 'target',
      concurrency: { scenarios: 2, judgments: 2 },
    },
  );

  await turn();
  assert.equal(started, 2);
  assert.equal(active, 2);

  gates[0]?.resolve(undefined);
  await turn();
  assert.equal(started, 3);
  assert.equal(active, 2);
  assert.equal(maximum, 2);

  blocking = false;
  for (const gate of gates) gate.resolve(undefined);
  await evaluation;
  assert.equal(started, 6);
  assert.equal(maximum, 2);
});

test('preserves sample and verdict order when calls complete out of order', async () => {
  const samples: ReturnType<typeof deferred<string>>[] = [];
  const judgments: {
    readonly key: string;
    readonly result: ReturnType<typeof deferred<ReturnType<typeof judgment>>>;
  }[] = [];
  const evaluation = evaluate(
    fakeCompletion(async () => {
      const result = deferred<string>();
      samples.push(result);
      return result.promise;
    }),
    {
      id: 'judge',
      completion: fakeCompletion(undefined, async (_system, input) => {
        const evalId = input.includes('Private rule a') ? 'a' : 'b';
        const sampleIndex = Number(
          input.match(/# Sample\n\nIndex: ([0-2])/u)?.[1],
        );
        const result = deferred<ReturnType<typeof judgment>>();
        judgments.push({
          key: `${evalId}:${sampleIndex}`,
          result,
        });
        return result.promise;
      }),
    },
    'prompt',
    [scenario('one', ['a', 'b'])],
    {
      targetId: 'target',
      concurrency: { scenarios: 1, judgments: 6 },
    },
  );

  await turn();
  assert.equal(samples.length, 3);
  samples[2]?.resolve('output-2');
  samples[1]?.resolve('output-1');
  samples[0]?.resolve('output-0');
  await turn();
  assert.equal(judgments.length, 6);
  for (const { result } of [...judgments].reverse()) {
    result.resolve(judgment());
  }

  const result = await evaluation;
  assert.deepEqual(result.results[0]?.outputs, [
    'output-0',
    'output-1',
    'output-2',
  ]);
  assert.deepEqual(
    result.results[0]?.verdicts.map(
      ({ evalId, sampleIndex }) => `${evalId}:${sampleIndex}`,
    ),
    ['a:0', 'a:1', 'a:2', 'b:0', 'b:1', 'b:2'],
  );
});

test('stops queued work on first failure and waits for active calls', async () => {
  const fail = deferred<void>();
  const settle = deferred<void>();
  const original = new Error('first judgment failed');
  const targetInputs: string[] = [];
  let judgeCalls = 0;
  let active = 0;
  const evaluation = evaluate(
    fakeCompletion(async (_system, input) => {
      targetInputs.push(input);
      return 'output';
    }),
    {
      id: 'judge',
      completion: fakeCompletion(undefined, async () => {
        const call = judgeCalls;
        judgeCalls += 1;
        active += 1;
        if (call === 0) {
          await fail.promise;
          active -= 1;
          throw original;
        }
        await settle.promise;
        active -= 1;
        return judgment();
      }),
    },
    'prompt',
    [scenario('one', ['a']), scenario('two', ['a']), scenario('three', ['a'])],
    {
      targetId: 'target',
      concurrency: { scenarios: 2, judgments: 2 },
      progress: ({ event }) => {
        if (event === 'scenario.failed') {
          throw new Error('progress failure must not replace provider failure');
        }
      },
    },
  );
  let rejection: unknown;
  let completed = false;
  const observed = evaluation.then(
    () => {
      completed = true;
    },
    (error: unknown) => {
      rejection = error;
      completed = true;
    },
  );

  await turn();
  assert.equal(judgeCalls, 2);
  assert.equal(active, 2);
  assert.equal(targetInputs.length, 6);

  fail.resolve(undefined);
  await turn();
  assert.equal(completed, false);
  assert.equal(judgeCalls, 2);
  assert.equal(targetInputs.includes('Private input three'), false);

  settle.resolve(undefined);
  await observed;
  assert.equal(rejection, original);
  assert.equal(active, 0);
  assert.equal(judgeCalls, 2);
  assert.equal(targetInputs.length, 6);
});
