import assert from 'node:assert/strict';
import test from 'node:test';

import type { CompletionFor } from '../src/completion.js';
import {
  evolutionStopReason,
  evolveModels,
  mergeScenarios,
} from '../src/evolve.js';
import type { EvolutionConfig } from '../src/schema.js';
import { fakeCompletion, validConfig } from './fakes.js';

test('stops for target, plateau, and hard cap in priority order', () => {
  const options = {
    targetAccuracy: 0.9,
    plateauPatience: 2,
    maxEpochs: 3,
  };
  assert.equal(evolutionStopReason(0.9, options, 0, 0), 'target-reached');
  assert.equal(evolutionStopReason(0.2, options, 1, 2), 'plateau');
  assert.equal(evolutionStopReason(0.2, options, 3, 0), 'max-epochs');
  assert.equal(evolutionStopReason(0.2, options, 1, 0), undefined);
});

test('rejects an empty baseline before creating model completions', async () => {
  let completionCount = 0;
  const completeFor: CompletionFor = () => {
    completionCount += 1;
    return fakeCompletion();
  };

  await assert.rejects(
    evolveModels(validConfig(), 'incumbent', [], completeFor),
    /initialize scenarios before evolving models/i,
  );
  assert.equal(completionCount, 0);
});

test('evolves every model independently from the original prompt', async () => {
  const config: EvolutionConfig = {
    ...validConfig(),
    models: [
      { id: 'model-a', provider: 'local', model: 'model-a' },
      { id: 'model-b', provider: 'local', model: 'model-b' },
    ],
    evolution: {
      targetAccuracy: 1,
      plateauPatience: 1,
      maxEpochs: 1,
    },
  };
  const contexts: string[] = [];
  const completeFor: CompletionFor = (model) => {
    if (model.model === 'optimizer-model') {
      return fakeCompletion(undefined, async (_system, input) => {
        contexts.push(input);
        const context = JSON.parse(input) as {
          readonly target: { readonly id: string };
        };
        return {
          prompt: 'better-for-' + context.target.id,
          scenarios: [],
          rationale: 'Fix the failure.',
        };
      });
    }
    if (model.model.startsWith('judge-')) {
      return fakeCompletion(undefined, async (_system, input) => {
        const value = JSON.parse(input) as { readonly output: string };
        return {
          passed: value.output.startsWith('better-for-'),
          ambiguous: false,
          rationale: 'Deterministic.',
        };
      });
    }
    return fakeCompletion(async (system) => system);
  };

  const results = await evolveModels(
    config,
    'original prompt',
    [{ id: 'case', input: 'input', expected: 'expected', tags: [] }],
    completeFor,
  );

  assert.deepEqual(
    results.map(({ prompt }) => prompt),
    ['better-for-model-a', 'better-for-model-b'],
  );
  assert.equal(contexts.length, 2);
  const first = JSON.parse(contexts[0] ?? '') as {
    readonly target: { readonly id: string };
    readonly prompt: string;
    readonly history: readonly { readonly failures: readonly unknown[] }[];
  };
  const second = JSON.parse(contexts[1] ?? '') as {
    readonly target: { readonly id: string };
    readonly prompt: string;
  };
  assert.equal(first.target.id, 'model-a');
  assert.equal(first.prompt, 'original prompt');
  assert.equal(first.history[0]?.failures.length, 1);
  assert.equal(contexts[0]?.includes('model-b'), false);
  assert.equal(second.target.id, 'model-b');
  assert.equal(second.prompt, 'original prompt');
  assert.equal(contexts[1]?.includes('model-a'), false);
});

test('preserves the incumbent prompt on an exact score tie', async () => {
  const config: EvolutionConfig = {
    ...validConfig(),
    evolution: {
      targetAccuracy: 1,
      plateauPatience: 1,
      maxEpochs: 1,
    },
  };
  const completeFor: CompletionFor = (model) =>
    model.model === 'optimizer-model'
      ? fakeCompletion(undefined, async () => ({
          prompt: 'candidate',
          scenarios: [],
          rationale: 'Try another prompt.',
        }))
      : model.model.startsWith('judge-')
        ? fakeCompletion(undefined, async () => ({
            passed: false,
            ambiguous: false,
            rationale: 'Still fails.',
          }))
        : fakeCompletion(async () => 'same output');
  const [result] = await evolveModels(
    config,
    'incumbent',
    [{ id: 'case', input: 'input', expected: 'expected', tags: [] }],
    completeFor,
  );
  assert.equal(result?.prompt, 'incumbent');
});

test('merges identical accepted scenarios and rejects conflicting ids', () => {
  const config = validConfig();
  const scenario = {
    id: 'new',
    input: 'input',
    expected: 'expected',
    tags: [],
  };
  const result = {
    target: config.models[0],
    prompt: 'prompt',
    evaluation: { score: 1, passed: 0, total: 0, results: [] },
    scenarios: [scenario],
    acceptedScenarios: [scenario],
    epochsRun: 1,
    stopReason: 'target-reached' as const,
  };
  assert.deepEqual(mergeScenarios([], [result, result]), [scenario]);
  assert.throws(() =>
    mergeScenarios(
      [],
      [
        result,
        {
          ...result,
          acceptedScenarios: [{ ...scenario, expected: 'different' }],
        },
      ],
    ),
  );
});
