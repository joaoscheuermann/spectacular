import assert from 'node:assert/strict';
import test from 'node:test';

import type { CompletionFor } from '../src/completion.js';
import { evolveModels } from '../src/evolve.js';
import type { HistoryRecord } from '../src/history.js';
import { compressionSystemPrompt } from '../src/prompts.js';
import { fakeCompletion, judgment, scenarios, validConfig } from './fakes.js';

const priorAttempt = (
  overrides: Partial<HistoryRecord> = {},
): HistoryRecord => ({
  fingerprint: 'matching-fingerprint',
  attemptedPrompt: 'historical training prompt',
  strategy: 'historical failed strategy',
  optimizerMode: 'normal',
  trainingAccuracy: 0,
  failedEvals: [],
  disposition: 'not-improved',
  terminalStatus: 'validation-failed',
  ...overrides,
});

test('uses two normal temperatures then one plateau escape temperature', async () => {
  const config = validConfig();
  const temperatures: (number | undefined)[] = [];
  let proposal = 0;
  const completeFor: CompletionFor = (model) => {
    if (model.model === 'optimizer-model') {
      temperatures.push(model.temperature);
      return fakeCompletion(undefined, async () => ({
        prompt: 'candidate-' + proposal++,
        strategy: 'strategy-' + proposal,
      }));
    }
    if (model.model === 'judge-model') {
      return fakeCompletion(undefined, async () => judgment(false));
    }
    return fakeCompletion(async () => 'bad output');
  };

  const [result] = await evolveModels(
    config,
    'original prompt',
    scenarios(),
    completeFor,
  );
  assert.deepEqual(temperatures, [0.2, 0.2, 0.8]);
  assert.equal(result?.stopReason, 'plateau');
  assert.equal(result?.epochsRun, 3);
});

test('accepts strict training improvement, compresses once, and isolates validation', async () => {
  const base = validConfig();
  const config = {
    ...base,
    providers: [
      ...base.providers,
      { id: 'target-provider-sentinel', type: 'lmstudio-openai' as const },
    ],
    models: [
      {
        id: 'target-id-sentinel',
        provider: 'target-provider-sentinel',
        model: 'target-model-sentinel',
      },
    ],
  };
  const optimizerRequests: { readonly system: string; readonly input: string }[] =
    [];
  const targetInputs: string[] = [];
  const goodPrompt = 'good ' + 'x'.repeat(95);
  const compressed = 'good ' + 'x'.repeat(70);
  let optimizerCalls = 0;
  let validationJudgments = 0;
  const completeFor: CompletionFor = (model) => {
    if (model.model === 'optimizer-model') {
      return fakeCompletion(undefined, async (system, input) => {
        optimizerRequests.push({ system, input });
        optimizerCalls += 1;
        return system === compressionSystemPrompt
          ? { prompt: compressed, strategy: 'remove repetition' }
          : { prompt: goodPrompt, strategy: 'make behavior explicit' };
      });
    }
    if (model.model === 'judge-model') {
      return fakeCompletion(undefined, async (_system, input) => {
        if (input.includes('validation input')) validationJudgments += 1;
        return judgment(input.includes('good '));
      });
    }
    return fakeCompletion(async (system, input) => {
      targetInputs.push(input);
      return system;
    });
  };

  const [result] = await evolveModels(
    config,
    'original prompt',
    scenarios(),
    completeFor,
  );
  assert.equal(optimizerCalls, 2);
  assert.equal(result?.approved, true);
  assert.equal(result?.refactored, true);
  assert.equal(result?.prompt, compressed);
  assert.equal(result?.stopReason, 'approved');
  assert.equal(validationJudgments, 3);
  const proposalInput = optimizerRequests.find(
    ({ system }) => system !== compressionSystemPrompt,
  )?.input;
  const compression = optimizerRequests.find(
    ({ system }) => system === compressionSystemPrompt,
  )?.input;
  assert.ok(proposalInput);
  assert.ok(compression);
  assert.deepEqual(proposalInput.match(/^# .+$/gmu), [
    '# Prompt',
    '# Training Scenarios',
    '# Current Failures',
    '# Matching History',
    '# Prohibited Prompts',
    '# Prohibited Strategies',
  ]);
  assert.deepEqual(compression.match(/^# .+$/gmu), [
    '# Prompt',
    '# Training Scenarios',
  ]);
  for (const input of [proposalInput, compression]) {
    assert.match(input, /training input/u);
    assert.match(input, /Always applies\./u);
    assert.equal(input.includes('validation input'), false);
    for (const identity of [
      'target-id-sentinel',
      'target-provider-sentinel',
      'target-model-sentinel',
    ]) {
      assert.equal(input.includes(identity), false);
    }
  }
  assert.deepEqual(targetInputs, [
    ...Array<string>(9).fill('training input'),
    ...Array<string>(3).fill('validation input'),
  ]);
});

test('continues validating structured optimizer responses with Markdown inputs', async () => {
  const config = {
    ...validConfig(),
    evolution: { ...validConfig().evolution, epochs: 1 },
  };
  const completeFor: CompletionFor = (model) => {
    if (model.model === 'optimizer-model') {
      return fakeCompletion(undefined, async () => ({ prompt: 'candidate' }));
    }
    if (model.model === 'judge-model') {
      return fakeCompletion(undefined, async () => judgment(false));
    }
    return fakeCompletion(async () => 'bad');
  };

  await assert.rejects(
    evolveModels(config, 'original', scenarios(), completeFor),
    (error: unknown) => {
      assert.ok(error instanceof AggregateError);
      const targetError = error.errors[0];
      assert.ok(targetError instanceof Error);
      assert.ok(targetError.cause instanceof Error);
      assert.equal(targetError.cause.name, 'ZodError');
      return true;
    },
  );
});

test('rejects equivalent prompts without target evaluation and respects max epochs', async () => {
  const config = {
    ...validConfig(),
    evolution: {
      ...validConfig().evolution,
      patience: { epochs: 5 },
      epochs: 1,
    },
  };
  let targetCalls = 0;
  const completeFor: CompletionFor = (model) => {
    if (model.model === 'optimizer-model') {
      return fakeCompletion(undefined, async () => ({
        prompt: '  ORIGINAL   prompt ',
        strategy: 'unchanged',
      }));
    }
    if (model.model === 'judge-model') {
      return fakeCompletion(undefined, async () => judgment(false));
    }
    return fakeCompletion(async () => {
      targetCalls += 1;
      return 'bad';
    });
  };
  const [result] = await evolveModels(
    config,
    'original prompt',
    scenarios(),
    completeFor,
  );
  assert.equal(targetCalls, 3);
  assert.equal(result?.stopReason, 'max-epochs');
  assert.equal(result?.history[0]?.disposition, 'duplicate-prompt');
});

test('omits Codex optimizer temperature and warns exactly once per run', async () => {
  const base = validConfig();
  const config = {
    ...base,
    providers: base.providers.map((provider) =>
      provider.id === 'optimizer'
        ? { ...provider, type: 'codex' as const }
        : provider,
    ),
    evolution: { ...base.evolution, epochs: 1 },
  };
  const warnings: string[] = [];
  const optimizerModels: object[] = [];
  const completeFor: CompletionFor = (model) => {
    if (model.model === 'optimizer-model') {
      optimizerModels.push(model);
      return fakeCompletion(undefined, async () => ({
        prompt: 'candidate',
        strategy: 'new',
      }));
    }
    if (model.model === 'judge-model') {
      return fakeCompletion(undefined, async () => judgment(false));
    }
    return fakeCompletion(async () => 'bad');
  };
  await evolveModels(config, 'original', scenarios(), completeFor, undefined, {
    warn: (message) => warnings.push(message),
  });
  assert.equal(warnings.length, 1);
  assert.equal(Object.hasOwn(optimizerModels[0] ?? {}, 'temperature'), false);
});

test('continues independent targets after one target throws', async () => {
  const base = validConfig();
  const config = {
    ...base,
    models: [
      { id: 'model-a', provider: 'local', model: 'target-a' },
      { id: 'model-b', provider: 'local', model: 'target-b' },
    ],
  };
  const completed: string[] = [];
  const completeFor: CompletionFor = (model) => {
    if (model.model === 'target-a') {
      return fakeCompletion(async () => {
        throw new Error('target-a failed');
      });
    }
    if (model.model === 'target-b') {
      return fakeCompletion(async () => 'good');
    }
    if (model.model === 'judge-model') {
      return fakeCompletion(undefined, async () => judgment());
    }
    return fakeCompletion(undefined, async () => ({
      prompt: 'x'.repeat(75),
      strategy: 'compress',
    }));
  };
  await assert.rejects(
    evolveModels(config, 'x'.repeat(100), scenarios(), completeFor, undefined, {
      onResult: (result) => {
        completed.push(result.target.id);
      },
    }),
    /targets failed/,
  );
  assert.deepEqual(completed, ['model-b']);
});

test('projects matching history to training-only feedback and rejects a failed strategy', async () => {
  const base = validConfig();
  const config = { ...base, evolution: { ...base.evolution, epochs: 1 } };
  const optimizerInputs: string[] = [];
  let targetCalls = 0;
  const completeFor: CompletionFor = (model) => {
    if (model.model === 'optimizer-model') {
      return fakeCompletion(undefined, async (_system, input) => {
        optimizerInputs.push(input);
        return {
          prompt: 'new candidate prompt',
          strategy: 'historical failed strategy',
        };
      });
    }
    if (model.model === 'judge-model') {
      return fakeCompletion(undefined, async () => judgment(false));
    }
    return fakeCompletion(async () => {
      targetCalls += 1;
      return 'bad';
    });
  };
  const [result] = await evolveModels(
    config,
    'original prompt',
    scenarios(),
    completeFor,
    undefined,
    {
      history: {
        model: [
          priorAttempt(),
          priorAttempt({
            attemptedPrompt: 'another training prompt',
            strategy: 'successful strategy',
            disposition: 'improved',
            terminalStatus: 'approved',
          }),
        ],
      },
    },
  );
  assert.equal(targetCalls, 3);
  assert.equal(result?.history[0]?.disposition, 'rejected-strategy');
  const optimizerInput = optimizerInputs[0] ?? '';
  assert.match(optimizerInput, /historical training prompt/);
  assert.match(optimizerInput, /historical failed strategy/);
  for (const heldOut of [
    'terminalStatus',
    'validation-failed',
    'approved',
    'validation input',
  ]) {
    assert.equal(optimizerInput.includes(heldOut), false);
  }
});

test('falls back without evaluating a compression candidate of invalid length', async () => {
  const config = validConfig();
  const original = 'x'.repeat(100);
  let targetCalls = 0;
  const completeFor: CompletionFor = (model) => {
    if (model.model === 'optimizer-model') {
      return fakeCompletion(undefined, async () => ({
        prompt: 'z'.repeat(90),
        strategy: 'insufficient compression',
      }));
    }
    if (model.model === 'judge-model') {
      return fakeCompletion(undefined, async (_system, input) => {
        return judgment(input.includes(original));
      });
    }
    return fakeCompletion(async (system) => {
      targetCalls += 1;
      return system;
    });
  };
  const [result] = await evolveModels(
    config,
    original,
    scenarios(),
    completeFor,
  );
  assert.equal(targetCalls, 6);
  assert.equal(result?.prompt, original);
  assert.equal(result?.refactored, false);
  assert.equal(result?.approved, true);
});

test('falls back when a correctly sized compression loses training accuracy', async () => {
  const config = validConfig();
  const original = 'x'.repeat(100);
  let targetCalls = 0;
  const completeFor: CompletionFor = (model) => {
    if (model.model === 'optimizer-model') {
      return fakeCompletion(undefined, async () => ({
        prompt: 'y'.repeat(75),
        strategy: 'unsafe compression',
      }));
    }
    if (model.model === 'judge-model') {
      return fakeCompletion(undefined, async (_system, input) => {
        return judgment(input.includes(original));
      });
    }
    return fakeCompletion(async (system) => {
      targetCalls += 1;
      return system;
    });
  };
  const [result] = await evolveModels(
    config,
    original,
    scenarios(),
    completeFor,
  );
  assert.equal(targetCalls, 9);
  assert.equal(result?.prompt, original);
  assert.equal(result?.refactored, false);
  assert.equal(result?.approved, true);
});

test('evaluates validation once and reports validation failure', async () => {
  const config = validConfig();
  let validationCalls = 0;
  const completeFor: CompletionFor = (model) => {
    if (model.model === 'optimizer-model') {
      return fakeCompletion(undefined, async () => ({
        prompt: 'z'.repeat(90),
        strategy: 'invalid compression',
      }));
    }
    if (model.model === 'judge-model') {
      return fakeCompletion(undefined, async (_system, input) => {
        if (input.includes('validation input')) validationCalls += 1;
        return judgment(input.includes('training input'));
      });
    }
    return fakeCompletion(async () => 'output');
  };
  const [result] = await evolveModels(
    config,
    'x'.repeat(100),
    scenarios(),
    completeFor,
  );
  assert.equal(validationCalls, 3);
  assert.equal(result?.approved, false);
  assert.equal(result?.validationAccuracy, 0);
  assert.equal(result?.stopReason, 'validation-failed');
});

test('returns to normal temperature after a successful plateau escape', async () => {
  const base = validConfig();
  const config = {
    ...base,
    evolution: {
      ...base.evolution,
      patience: { epochs: 1 },
      epochs: 3,
    },
  };
  const temperatures: (number | undefined)[] = [];
  let proposal = 0;
  const completeFor: CompletionFor = (model) => {
    if (model.model === 'optimizer-model') {
      temperatures.push(model.temperature);
      proposal += 1;
      return fakeCompletion(undefined, async () => ({
        prompt: 'candidate-' + proposal,
        strategy: 'strategy-' + proposal,
      }));
    }
    if (model.model === 'judge-model') {
      return fakeCompletion(undefined, async (_system, input) => {
        return judgment(
          input.includes('candidate-2') &&
            /# Sample\n\nIndex: 0(?:\n|$)/u.test(input),
        );
      });
    }
    return fakeCompletion(async (system) => system);
  };
  const [result] = await evolveModels(
    config,
    'original',
    scenarios(),
    completeFor,
  );
  assert.deepEqual(temperatures, [0.2, 0.8, 0.2]);
  assert.equal(result?.stopReason, 'max-epochs');
});
