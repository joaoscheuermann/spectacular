import assert from 'node:assert/strict';
import test from 'node:test';
import { ZodError } from 'zod';

import type { Judge } from '../src/evaluate.js';
import { initializeScenarios } from '../src/scenario-initialization.js';
import type { ProgressEvent } from '../src/progress.js';
import { fakeCompletion } from './fakes.js';

const candidate = (id: string) => ({
  id,
  input: 'Private input for ' + id,
  expected: 'Private expectation for ' + id,
  rationale: 'Private candidate rationale for ' + id,
  tags: ['generated'],
});

const judges = (
  decide: (input: string) => {
    readonly passed: boolean;
    readonly ambiguous?: boolean;
    readonly rationale: string;
  },
): readonly Judge[] =>
  ['judge-one', 'judge-two'].map((id) => ({
    id,
    completion: fakeCompletion(undefined, async (_system, input) =>
      decide(input),
    ),
  }));

test('rejects an empty structured initialization batch', async () => {
  let judgeCalls = 0;

  await assert.rejects(
    initializeScenarios({
      optimizer: fakeCompletion(undefined, async () => ({
        scenarios: [],
        rationale: 'Private empty-batch rationale.',
      })),
      judges: judges(() => {
        judgeCalls += 1;
        return { passed: true, rationale: 'Private judge rationale.' };
      }),
      originalPrompt: 'Private original prompt.',
      maxAttempts: 2,
    }),
    (error: unknown) => {
      assert.ok(error instanceof ZodError);
      assert.equal(error.issues[0]?.code, 'too_small');
      assert.deepEqual(error.issues[0]?.path, ['scenarios']);
      return true;
    },
  );
  assert.equal(judgeCalls, 0);
});

test('retries an all-rejected batch with complete feedback and returns the next accepted batch', async () => {
  const optimizerInputs: string[] = [];
  let attempt = 0;
  const accepted = candidate('accepted-case');
  const alsoAccepted = candidate('also-accepted-case');

  const scenarios = await initializeScenarios({
    optimizer: fakeCompletion(undefined, async (_system, input) => {
      optimizerInputs.push(input);
      attempt += 1;
      return {
        scenarios:
          attempt === 1
            ? [candidate('rejected-case')]
            : [accepted, alsoAccepted],
        rationale: 'Private optimizer rationale.',
      };
    }),
    judges: judges((input) => {
      const value = JSON.parse(input) as { readonly input: string };
      const passed =
        value.input === accepted.input || value.input === alsoAccepted.input;
      return {
        passed,
        ambiguous: false,
        rationale: passed
          ? 'Private accepted rationale.'
          : 'Private rejected rationale.',
      };
    }),
    originalPrompt: 'Private original prompt.',
    maxAttempts: 2,
  });

  assert.deepEqual(scenarios, [
    {
      id: accepted.id,
      input: accepted.input,
      expected: accepted.expected,
      rationale: accepted.rationale,
      tags: accepted.tags,
    },
    {
      id: alsoAccepted.id,
      input: alsoAccepted.input,
      expected: alsoAccepted.expected,
      rationale: alsoAccepted.rationale,
      tags: alsoAccepted.tags,
    },
  ]);
  assert.equal(optimizerInputs.length, 2);
  const feedback = JSON.parse(optimizerInputs[1] ?? '') as {
    readonly feedback: {
      readonly attemptedCandidates: readonly {
        readonly id: string;
        readonly input: string;
      }[];
      readonly rejections: readonly {
        readonly scenarioId: string;
        readonly reason: string;
        readonly judgments: readonly {
          readonly judge: string;
          readonly passed: boolean;
          readonly ambiguous: boolean;
          readonly rationale: string;
        }[];
      }[];
    };
  };
  assert.equal(feedback.feedback.attemptedCandidates[0]?.id, 'rejected-case');
  assert.match(
    feedback.feedback.attemptedCandidates[0]?.input ?? '',
    /Private input/,
  );
  assert.deepEqual(feedback.feedback.rejections, [
    {
      scenarioId: 'rejected-case',
      reason: 'rejected',
      judgments: [
        {
          judge: 'judge-one',
          passed: false,
          ambiguous: false,
          rationale: 'Private rejected rationale.',
        },
        {
          judge: 'judge-two',
          passed: false,
          ambiguous: false,
          rationale: 'Private rejected rationale.',
        },
      ],
    },
  ]);
});

test('fails after exactly the configured number of rejected attempts without exposing bodies', async () => {
  let optimizerCalls = 0;
  const progress: ProgressEvent[] = [];
  const privatePrompt = 'Private exhaustion prompt.';
  const privateJudgeRationale = 'Private exhaustion judge rationale.';

  await assert.rejects(
    initializeScenarios({
      optimizer: fakeCompletion(undefined, async () => {
        optimizerCalls += 1;
        return {
          scenarios: [candidate('rejected-' + optimizerCalls)],
          rationale: 'Private exhaustion optimizer rationale.',
        };
      }),
      judges: judges(() => ({
        passed: false,
        ambiguous: false,
        rationale: privateJudgeRationale,
      })),
      originalPrompt: privatePrompt,
      maxAttempts: 3,
      progress: (event) => progress.push(event),
    }),
    (error: unknown) => {
      assert.ok(error instanceof Error);
      assert.match(error.message, /initialization/i);
      assert.match(error.message, /3 attempts/i);
      assert.equal(error.message.includes(privatePrompt), false);
      assert.equal(error.message.includes(privateJudgeRationale), false);
      assert.equal(error.message.includes('Private input'), false);
      return true;
    },
  );
  assert.equal(optimizerCalls, 3);
  assert.deepEqual(progress.at(-1), {
    event: 'scenario-initialization.failed',
    attemptCount: 3,
  });
  assert.equal(JSON.stringify(progress).includes(privatePrompt), false);
  assert.equal(JSON.stringify(progress).includes(privateJudgeRationale), false);
});

test('reports initialization progress without prompt, scenario, or rationale bodies', async () => {
  const progress: ProgressEvent[] = [];
  const privateBodies = [
    'Private progress prompt.',
    'Private input for accepted-case',
    'Private expectation for accepted-case',
    'Private candidate rationale for accepted-case',
    'Private progress optimizer rationale.',
    'Private progress judge rationale.',
  ];

  await initializeScenarios({
    optimizer: fakeCompletion(undefined, async () => ({
      scenarios: [candidate('accepted-case')],
      rationale: privateBodies[4],
    })),
    judges: judges(() => ({
      passed: true,
      ambiguous: false,
      rationale: privateBodies[5] ?? '',
    })),
    originalPrompt: privateBodies[0] ?? '',
    maxAttempts: 2,
    progress: (event) => progress.push(event),
  });

  assert.deepEqual(
    progress.filter(({ event }) => event.startsWith('scenario-initialization')),
    [
      { event: 'scenario-initialization.start', maxAttemptCount: 2 },
      { event: 'scenario-initialization.attempt.start', attempt: 1 },
      {
        event: 'scenario-initialization.attempt.complete',
        attempt: 1,
        candidateCount: 1,
        acceptedCount: 1,
        rejectedCount: 0,
      },
      {
        event: 'scenario-initialization.complete',
        attemptCount: 1,
        scenarioCount: 1,
      },
    ],
  );
  const logged = JSON.stringify(progress);
  for (const body of privateBodies) {
    assert.equal(logged.includes(body), false);
  }
});
