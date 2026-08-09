import assert from 'node:assert/strict';
import test from 'node:test';

import type { LlmProvider, Model } from 'llms';

import {
  paidCapabilityProbes,
  validateFreeCapabilities,
} from '../src/cli/preflight.js';
import {
  EMBEDDING_MODEL,
  PRIMARY_MODEL,
  RERANKER_MODEL,
} from '../src/config/index.js';

const completion = (id: string): Model => ({
  id,
  raw: {
    supported_parameters: ['tools', 'structured_outputs', 'reasoning'],
  },
});

const provider = (
  models: readonly Model[],
  calls: { embedding: number; rerank: number },
): LlmProvider =>
  ({
    metadata: { id: 'fake', name: 'Fake', baseUrl: 'https://example.test' },
    capabilities: {
      streaming: true,
      embeddings: true,
      reranking: true,
      tools: true,
      reasoning: true,
      modelListing: true,
      oauth: false,
      serviceTier: false,
      structuredOutputs: true,
    },
    models: async () => models,
    validateModel: async () => {
      throw new Error('unused');
    },
    complete: async () => {
      throw new Error('unused');
    },
    stream: async function* () {
      return;
    },
    embedding: async () => {
      calls.embedding += 1;
      return Array.from({ length: EMBEDDING_MODEL.dimensions }, () => 0);
    },
    rerank: async () => {
      calls.rerank += 1;
      return [{ index: 0, relevanceScore: 1 }];
    },
  }) as unknown as LlmProvider;

test('free preflight verifies configured IDs and capabilities without a paid probe', async () => {
  const calls = { embedding: 0, rerank: 0 };
  const candidate = 'provider/configured-candidate';
  const report = await validateFreeCapabilities(
    provider(
      [
        completion(PRIMARY_MODEL.model),
        completion(candidate),
        { id: EMBEDDING_MODEL.model },
        { id: RERANKER_MODEL },
      ],
      calls,
    ),
    candidate,
  );
  assert.deepEqual(report.paidProbesRequired, ['embedding', 'rerank']);
  assert.deepEqual(calls, { embedding: 0, rerank: 0 });
});

test('free preflight fails before paid work when model metadata is incomplete', async () => {
  const calls = { embedding: 0, rerank: 0 };
  const candidate = 'provider/configured-candidate';
  await assert.rejects(
    validateFreeCapabilities(
      provider(
        [
          completion(PRIMARY_MODEL.model),
          { id: candidate, raw: { supported_parameters: ['tools'] } },
          { id: EMBEDDING_MODEL.model },
          { id: RERANKER_MODEL },
        ],
        calls,
      ),
      candidate,
    ),
    /structured output/u,
  );
  assert.deepEqual(calls, { embedding: 0, rerank: 0 });
});

test('paid probes are explicit functions that verify dimensions and rerank shape', async () => {
  const calls = { embedding: 0, rerank: 0 };
  const probes = paidCapabilityProbes(provider([], calls));
  assert.deepEqual(calls, { embedding: 0, rerank: 0 });
  await probes.embedding();
  await probes.rerank();
  assert.deepEqual(calls, { embedding: 1, rerank: 1 });
});
