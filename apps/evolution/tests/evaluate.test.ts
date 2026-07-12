import assert from 'node:assert/strict';
import test from 'node:test';

import { evaluate, type Judge } from '../src/evaluate.js';
import type { ProgressEvent } from '../src/progress.js';
import { validateScenarioCandidates } from '../src/scenarios.js';
import { fakeCompletion } from './fakes.js';

const judges = (
  values: readonly {
    readonly passed: boolean;
    readonly ambiguous?: boolean;
  }[],
): readonly Judge[] =>
  values.map((value, index) => ({
    id: 'judge-' + index,
    completion: fakeCompletion(undefined, async () => ({
      passed: value.passed,
      ambiguous: value.ambiguous ?? false,
      rationale: 'Private rationale.',
    })),
  }));

test('evaluates generic text with unanimous judges and body-free progress', async () => {
  const targetInputs: string[] = [];
  const judgeInputs: string[] = [];
  const progress: ProgressEvent[] = [];
  const result = await evaluate(
    fakeCompletion(async (_system, input) => {
      targetInputs.push(input);
      return 'Private output';
    }),
    [0, 1].map((index) => ({
      id: 'judge-' + index,
      completion: fakeCompletion(undefined, async (_system, input) => {
        judgeInputs.push(input);
        return {
          passed: true,
          ambiguous: false,
          rationale: 'Private rationale.',
        };
      }),
    })),
    'Private system prompt',
    [
      {
        id: 'case-one',
        input: 'Private scenario input',
        expected: 'Private expected behavior',
        tags: [],
      },
    ],
    {
      targetId: 'target-one',
      progress: (event) => progress.push(event),
    },
  );

  assert.equal(result.score, 1);
  assert.deepEqual(targetInputs, ['Private scenario input']);
  assert.equal(judgeInputs.length, 2);
  assert.match(judgeInputs[0] ?? '', /Private expected behavior/);
  const logged = JSON.stringify(progress);
  for (const body of [
    'Private system prompt',
    'Private scenario input',
    'Private expected behavior',
    'Private output',
    'Private rationale',
  ]) {
    assert.equal(logged.includes(body), false);
  }
});

test('fails an evaluation when any judge rejects or is ambiguous', async () => {
  const scenario = [
    { id: 'case-one', input: 'input', expected: 'expected', tags: [] },
  ];
  const rejected = await evaluate(
    fakeCompletion(async () => 'output'),
    judges([{ passed: true }, { passed: false }]),
    'prompt',
    scenario,
    { targetId: 'target' },
  );
  const ambiguous = await evaluate(
    fakeCompletion(async () => 'output'),
    judges([{ passed: true }, { passed: true, ambiguous: true }]),
    'prompt',
    scenario,
    { targetId: 'target' },
  );
  assert.equal(rejected.score, 0);
  assert.equal(ambiguous.score, 0);
});

test('rejects duplicates before judging and accepts unanimous candidates', async () => {
  let calls = 0;
  const payloads: string[] = [];
  const candidateJudges: readonly Judge[] = [0, 1].map((index) => ({
    id: 'judge-' + index,
    completion: fakeCompletion(undefined, async (_system, input) => {
      calls += 1;
      payloads.push(input);
      return { passed: true, ambiguous: false, rationale: 'Clear.' };
    }),
  }));
  const results = await validateScenarioCandidates({
    judges: candidateJudges,
    originalPrompt: 'Original task contract',
    incumbents: [
      { id: 'existing', input: ' Add   dark mode ', expected: 'x', tags: [] },
    ],
    candidates: [
      { id: 'duplicate', input: 'add DARK mode', expected: 'x', tags: [] },
      { id: 'new-case', input: 'Export PDF', expected: 'PDF output', tags: [] },
    ],
  });
  assert.equal(results[0]?.reason, 'duplicate-input');
  assert.equal(results[1]?.accepted, true);
  assert.equal(calls, 2);
  assert.ok(
    payloads.every((payload) =>
      payload.includes('"originalPrompt":"Original task contract"'),
    ),
  );
});

test('requires two independent unanimous and unambiguous scenario judges', async () => {
  const candidate = [
    { id: 'case', input: 'input', expected: 'expected', tags: [] },
  ];
  const insufficient = await validateScenarioCandidates({
    judges: judges([{ passed: true }]),
    originalPrompt: 'Original task contract',
    incumbents: [],
    candidates: candidate,
  });
  const disagreement = await validateScenarioCandidates({
    judges: judges([{ passed: true }, { passed: false }]),
    originalPrompt: 'Original task contract',
    incumbents: [],
    candidates: candidate,
  });
  const ambiguous = await validateScenarioCandidates({
    judges: judges([{ passed: true }, { passed: true, ambiguous: true }]),
    originalPrompt: 'Original task contract',
    incumbents: [],
    candidates: candidate,
  });
  assert.equal(insufficient[0]?.reason, 'insufficient-judges');
  assert.equal(disagreement[0]?.reason, 'judge-disagreement');
  assert.equal(ambiguous[0]?.reason, 'ambiguous');
});
