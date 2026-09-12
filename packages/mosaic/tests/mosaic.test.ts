import assert from 'node:assert/strict';
import test from 'node:test';

import mosaicDefault, {
  mosaic,
  type MosaicAgent,
  type MosaicOptions,
} from '../src/index.js';
import { mosaicProviders } from './structured.js';

test('exports the same factory as named and default with an async prompt', () => {
  assert.strictEqual(mosaicDefault, mosaic);

  const agent: MosaicAgent = {
    prompt: async () => ({
      status: 'completed',
      delivery: { markdown: 'Done.', parts: [] },
      nodes: [],
    }),
  };

  assert.ok(agent.prompt('request') instanceof Promise);
});

test('propagates the exact provider failure from graph generation', async () => {
  const failure = new Error('provider unavailable');

  const options: MosaicOptions = {
    logger: { info: () => undefined, debug: () => undefined } as never,
    providers: mosaicProviders({
      metadata: {
        id: 'fake',
        name: 'Fake',
        baseUrl: 'https://fake.invalid',
      },
      complete: async () => {
        throw failure;
      },
    } as never),
    models: {
      planning: { model: 'default-model', effort: 'low' },
      revision: { model: 'default-model', effort: 'low' },
      execution: { model: 'default-model', effort: 'low' },
      reranker: 'reranker-model',
      embedder: 'embedder-model',
    },
    routing: {
      maxHintCandidates: 5,
      maxRetrievedCandidates: 5,
      maxSkills: 5,
    },
    execution: { maxTurns: 8 },
    revision: { max: 3 },
    skills: {
      required: [],
      menu: [],
      retriever: { search: async () => [] },
    },
    tools: {
      required: [],
      menu: [],
      retriever: { search: async () => [] },
    },
  };

  await assert.rejects(mosaic(options).prompt('Do it.'), (error) => {
    assert.strictEqual(error, failure);

    return true;
  });
});
