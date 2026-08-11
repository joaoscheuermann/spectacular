import assert from 'node:assert/strict';
import test from 'node:test';

import { mosaic as publicMosaic, type MosaicOptions } from '../src/index.js';
import { mosaic as evaluationMosaic } from '../src/evaluation.js';
import type { LlmProvider, ProviderRequest } from 'llms';

import { mosaicProviders, terminalFinish } from './structured.js';

test('evaluation adapter without hooks is behaviorally neutral', async () => {
  const first = createHarness();
  const second = createHarness();

  const publicResult = await publicMosaic(first.options).prompt('request');
  const evaluationResult = await evaluationMosaic(second.options).prompt(
    'request',
  );

  assert.deepEqual(evaluationResult, publicResult);
  assert.deepEqual(
    second.requests.map(projectRequest),
    first.requests.map(projectRequest),
  );
});

test('routes planning and execution through their configured profiles', async () => {
  const harness = createHarness();
  const options: MosaicOptions = {
    ...harness.options,
    models: {
      planning: { model: 'planning-model', effort: 'medium' },
      revision: { model: 'revision-model', effort: 'high' },
      execution: { model: 'execution-model', effort: 'low' },
      reranker: 'reranker-model',
      embedder: 'embedder-model',
    },
  };

  await publicMosaic(options).prompt('request');

  assert.deepEqual(
    harness.requests.map(({ model, effort }) => ({ model, effort })),
    [
      { model: 'planning-model', effort: 'medium' },
      { model: 'planning-model', effort: 'medium' },
      { model: 'execution-model', effort: 'low' },
    ],
  );
});

test('feedback unchanged skips hints and P1 while materializing revision one', async () => {
  const harness = createHarness(true);
  let frozen = false;

  const result = await evaluationMosaic(harness.options, {
    hooks: {
      feedbackPlan: async (input) => {
        frozen = Object.isFrozen(input) && Object.isFrozen(input.graph);
        return 'unchanged';
      },
    },
  }).prompt('request');

  assert.equal(result.status, 'completed');
  assert.equal(frozen, true);
  assert.equal(harness.requests.length, 2);
  assert.equal(harness.searches.length, 1);
});

test('invalid hook output is rejected before graph state is committed', async () => {
  const harness = createHarness();

  await assert.rejects(
    evaluationMosaic(harness.options, {
      hooks: {
        initialPlan: async () => ({ nodes: [] }),
      },
    }).prompt('request'),
  );
  assert.equal(harness.requests.length, 0);
});

const projectRequest = (request: ProviderRequest<unknown>) => ({
  model: request.model,
  messages: request.messages,
  tools: request.tools,
  effort: request.effort,
});

const plan = {
  nodes: [
    {
      id: 'n01:finish',
      goal: 'Produce the result.',
      doneWhen: ['The result is complete.'],
      dependsOn: [],
      deliver: true,
    },
  ],
};

const decision = {
  status: 'completed',
  criteria: [
    {
      criterionIndex: 0,
      satisfied: true,
      evidence: 'Result produced.',
      observationIndices: [],
    },
  ],
  result: { markdown: 'Done.', artifacts: [] },
  revisionRequest: null,
  reason: null,
};

const createHarness = (withSearch = false) => {
  const requests: ProviderRequest<unknown>[] = [];
  const searches: string[] = [];
  const provider = {
    metadata: { id: 'fake', name: 'Fake', baseUrl: 'https://fake.invalid' },
    complete: async (request: ProviderRequest<unknown>) => {
      requests.push(request);
      const system = request.messages[0]?.content;
      const value =
        typeof system === 'string' &&
        system.startsWith('You execute one outcome-oriented node')
          ? decision
          : plan;
      return terminalFinish(request, value);
    },
  } as unknown as LlmProvider;
  const options: MosaicOptions = {
    logger: { info: () => undefined, debug: () => undefined } as never,
    providers: mosaicProviders(provider),
    models: {
      planning: { model: 'default-model', effort: 'low' },
      revision: { model: 'default-model', effort: 'low' },
      execution: { model: 'default-model', effort: 'low' },
      reranker: 'reranker-model',
      embedder: 'embedder-model',
    },
    routing: {
      maxHintCandidates: 3,
      maxRetrievedCandidates: 3,
      maxSkills: 3,
    },
    execution: { maxTurns: 4 },
    revision: { max: 1 },
    skills: {
      required: [],
      menu: withSearch
        ? [
            {
              name: 'skill',
              description: 'Skill metadata.',
              body: 'Skill body.',
              allowedTools: [],
              indexText: 'skill | Skill metadata. | | Skill body.',
            },
          ]
        : [],
      retriever: {
        search: async (query) => {
          searches.push(query);
          return [];
        },
      },
    },
    tools: {
      required: [],
      menu: [],
      retriever: { search: async () => [] },
    },
  };

  return { options, requests, searches };
};
