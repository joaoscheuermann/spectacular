import assert from 'node:assert/strict';
import test from 'node:test';

import {
  compressionInput,
  judgeInput,
  optimizerInput,
} from '../src/prompts.js';

test('renders optimizer sections in order with complete nested training and history data', () => {
  const input = optimizerInput({
    prompt: 'System prompt',
    trainingScenarios: [
      {
        id: 'scenario-one',
        split: 'train',
        input: 'First line\n# Authored heading\nCafé 🌍',
        evals: [
          { id: 'rule-one', assertion: 'Return “valid”.' },
          { id: 'rule-two', assertion: 'Keep\nmultiple lines.' },
        ],
        rationale: 'Why this matters.',
      },
      {
        id: 'scenario-two',
        split: 'train',
        input: 'No rationale here.',
        evals: [],
      },
    ],
    currentFailures: [
      {
        scenarioId: 'scenario-one',
        evalId: 'rule-two',
        sampleIndex: 2,
        passed: false,
        output: 'Wrong output',
        reasoning: 'Missed the second line.',
      },
    ],
    history: [
      {
        attemptedPrompt: 'Earlier prompt',
        strategy: 'Clarify formatting',
        optimizerMode: 'plateau-escape',
        trainingAccuracy: 0.75,
        failedEvals: [
          {
            scenarioId: 'historical-case',
            evalId: 'historical-rule',
            sampleIndex: 1,
            passed: false,
            output: 'Historical output',
            reasoning: 'Historical reason',
          },
        ],
        disposition: 'not-improved',
      },
      {
        attemptedPrompt: 'Successful history prompt',
        strategy: 'Successful strategy',
        optimizerMode: 'normal',
        trainingAccuracy: 1,
        failedEvals: [],
        disposition: 'improved',
      },
    ],
    prohibitedPrompts: ['Earlier prompt', 'Earlier prompt'],
    prohibitedStrategies: ['Clarify formatting', 'Clarify formatting'],
  });

  assert.deepEqual(input.match(/^# .+$/gmu), [
    '# Prompt',
    '# Training Scenarios',
    '# Authored heading',
    '# Current Failures',
    '# Matching History',
    '# Prohibited Prompts',
    '# Prohibited Strategies',
  ]);
  assert.match(input, /## Scenario 1/u);
  assert.match(input, /## Scenario 2/u);
  assert.match(input, /ID:\n\n```\nscenario-one\n```/u);
  assert.match(input, /Split:\n\n```\ntrain\n```/u);
  assert.match(input, /Rationale:\n\n```\nWhy this matters\.\n```/u);
  assert.equal(input.match(/Rationale:/gu)?.length, 1);
  assert.match(input, /Assertions:\n\nNone\./u);
  assert.match(input, /## Attempt 1/u);
  assert.match(input, /## Attempt 2/u);
  assert.match(input, /### Failure 1/u);
  assert.match(input, /Scenario ID:\n\n```\nscenario-one\n```/u);
  assert.match(input, /Eval ID:\n\n```\nrule-two\n```/u);
  assert.match(input, /Sample Index: 2/u);
  assert.match(input, /Passed: false/u);
  assert.match(input, /Output:\n\n```\nWrong output\n```/u);
  assert.match(
    input,
    /Reasoning:\n\n```\nMissed the second line\.\n```/u,
  );
  assert.match(input, /Optimizer Mode:\n\n```\nplateau-escape\n```/u);
  assert.match(input, /Training Accuracy: 0\.75/u);
  assert.match(input, /Disposition:\n\n```\nnot-improved\n```/u);
  assert.match(
    input,
    /## Attempt 2[\s\S]*?Failed Evaluations:\n\nNone\.[\s\S]*?Disposition:\n\n```\nimproved\n```/u,
  );
  assert.match(input, /Historical output/u);
  assert.match(input, /Historical reason/u);
  assert.match(input, /Scenario ID:\n\n```\nhistorical-case\n```/u);
  assert.match(input, /Eval ID:\n\n```\nhistorical-rule\n```/u);
  assert.match(input, /Sample Index: 1/u);
  assert.equal(input.match(/Earlier prompt/gu)?.length, 3);
  assert.equal(input.match(/Clarify formatting/gu)?.length, 3);
  assert.match(input, /Café 🌍/u);
  assert.match(input, /Keep\nmultiple lines\./u);
});

test('renders empty optimizer collections as None', () => {
  const input = optimizerInput({
    prompt: 'Prompt',
    trainingScenarios: [],
    currentFailures: [],
    history: [],
    prohibitedPrompts: [],
    prohibitedStrategies: [],
  });

  assert.equal(
    input,
    [
      '# Prompt',
      '',
      '```',
      'Prompt',
      '```',
      '',
      '# Training Scenarios',
      '',
      'None.',
      '',
      '# Current Failures',
      '',
      'None.',
      '',
      '# Matching History',
      '',
      'None.',
      '',
      '# Prohibited Prompts',
      '',
      'None.',
      '',
      '# Prohibited Strategies',
      '',
      'None.',
    ].join('\n'),
  );
});

test('reuses scenario rendering for compression input', () => {
  const scenario = {
    id: 'shared-case',
    split: 'train' as const,
    input: 'Shared input',
    evals: [{ id: 'shared-rule', assertion: 'Shared assertion' }],
    rationale: 'Shared rationale',
  };
  const optimizer = optimizerInput({
    prompt: 'Long prompt',
    trainingScenarios: [scenario],
    currentFailures: [],
    history: [],
    prohibitedPrompts: [],
    prohibitedStrategies: [],
  });
  const compression = compressionInput({
    prompt: 'Long prompt',
    trainingScenarios: [scenario],
  });

  const optimizerScenarios = optimizer.slice(
    optimizer.indexOf('# Training Scenarios'),
    optimizer.indexOf('\n\n# Current Failures'),
  );
  const compressionScenarios = compression.slice(
    compression.indexOf('# Training Scenarios'),
  );
  assert.equal(compressionScenarios, optimizerScenarios);
  assert.deepEqual(compression.match(/^# .+$/gmu), [
    '# Prompt',
    '# Training Scenarios',
  ]);
});

test('renders judge pairing and chooses collision-safe fences deterministically', () => {
  const input = judgeInput({
    scenarioInput: 'Ticks ````` and tildes ~~~',
    assertion: {
      id: 'rule-```-~~~~~-id',
      assertion: 'Both `` and ~~ tie.',
    },
    sampleIndex: 2,
    modelOutput: 'Ends without newline and contains ````',
  });

  assert.equal(
    input,
    [
      '# Scenario Input',
      '',
      '~~~~',
      'Ticks ````` and tildes ~~~',
      '~~~~',
      '',
      '# Assertion',
      '',
      'ID:',
      '',
      '````',
      'rule-```-~~~~~-id',
      '````',
      '',
      'Text:',
      '',
      '```',
      'Both `` and ~~ tie.',
      '```',
      '',
      '# Sample',
      '',
      'Index: 2',
      '',
      '# Model Output',
      '',
      '~~~',
      'Ends without newline and contains ````',
      '~~~',
    ].join('\n'),
  );
});

test('does not add an extra body line when fenced text already ends in a newline', () => {
  const input = compressionInput({
    prompt: 'line one\n',
    trainingScenarios: [],
  });

  assert.match(input, /^# Prompt\n\n```\nline one\n```\n\n/u);
});
