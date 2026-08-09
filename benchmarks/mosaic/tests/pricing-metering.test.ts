import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import {
  createOpenAiProvider,
  type HttpRequest,
  type HttpResponse,
  type HttpTransport,
} from 'llms';
import pino from 'pino';

import { createMeteringStore } from '../src/cli/metering-store.js';
import { operationCost, parsePrices } from '../src/cli/pricing.js';
import { createMeteredProvider } from '../src/cli/provider.js';
import { createUsageTransport } from '../src/cli/usage-transport.js';

const response = (body: unknown, status = 200): HttpResponse => ({
  status,
  headers: {},
  body: JSON.stringify(body),
});

const transport = (responses: readonly HttpResponse[]): HttpTransport => {
  let offset = 0;
  return {
    request: async (_request: HttpRequest) =>
      responses[offset++] ?? response({ error: 'missing fixture' }, 500),
    async *stream() {
      return;
    },
  };
};

const prices = () =>
  parsePrices({
    schemaVersion: 1,
    currency: 'USD',
    models: {
      completion: {
        kind: 'completion',
        capturedAt: '2026-08-09T00:00:00.000Z',
        source: 'https://example.test/completion',
        charges: [
          { unit: 'input-token', quantity: 1_000, priceUsd: 2 },
          { unit: 'cached-input-token', quantity: 1_000, priceUsd: 1 },
          { unit: 'output-token', quantity: 1_000, priceUsd: 4 },
        ],
      },
      embedding: {
        kind: 'embedding',
        capturedAt: '2026-08-09T00:00:00.000Z',
        source: 'https://example.test/embedding',
        charges: [
          { unit: 'embedding-input-token', quantity: 1_000, priceUsd: 0.2 },
        ],
      },
      rerank: {
        kind: 'rerank',
        capturedAt: '2026-08-09T00:00:00.000Z',
        source: 'https://example.test/rerank',
        charges: [
          { unit: 'rerank-input-token', quantity: 1_000, priceUsd: 0.5 },
          { unit: 'rerank-search-unit', quantity: 1, priceUsd: 0.01 },
        ],
      },
    },
  });

test('price contract rejects zero, wrong currency, inapplicable units, and incomplete sources', () => {
  const model = {
    kind: 'embedding',
    capturedAt: '2026-08-09T00:00:00.000Z',
    source: 'https://example.test/embedding',
    charges: [
      { unit: 'embedding-input-token', quantity: 1_000, priceUsd: 0.2 },
    ],
  };
  assert.throws(() =>
    parsePrices({ schemaVersion: 1, currency: 'EUR', models: { model } }),
  );
  assert.throws(() =>
    parsePrices({
      schemaVersion: 1,
      currency: 'USD',
      models: {
        model: {
          ...model,
          charges: [{ ...model.charges[0], priceUsd: 0 }],
        },
      },
    }),
  );
  assert.throws(() =>
    parsePrices({
      schemaVersion: 1,
      currency: 'USD',
      models: {
        model: {
          ...model,
          charges: [{ unit: 'output-token', quantity: 1, priceUsd: 1 }],
        },
      },
    }),
  );
  assert.throws(() =>
    parsePrices({
      schemaVersion: 1,
      currency: 'USD',
      models: { model: { ...model, source: 'not-a-source' } },
    }),
  );
});

test('price tiers must cover a contiguous range and select exactly one rate', () => {
  const tiered = parsePrices({
    schemaVersion: 1,
    currency: 'USD',
    models: {
      model: {
        kind: 'completion',
        capturedAt: '2026-08-09T00:00:00.000Z',
        source: 'https://example.test/model',
        charges: [
          {
            unit: 'input-token',
            quantity: 1_000,
            priceUsd: 1,
            tier: {
              id: 'short',
              basis: 'input-tokens',
              minimumInclusive: 0,
              maximumExclusive: 1_000,
            },
          },
          {
            unit: 'input-token',
            quantity: 1_000,
            priceUsd: 2,
            tier: {
              id: 'long',
              basis: 'input-tokens',
              minimumInclusive: 1_000,
            },
          },
        ],
      },
    },
  });
  assert.equal(
    operationCost(tiered, 'model', { inputTokens: 1_500, requests: 1 }),
    3,
  );
  assert.throws(() =>
    parsePrices({
      schemaVersion: 1,
      currency: 'USD',
      models: {
        model: {
          ...tiered.models['model'],
          charges: tiered.models['model']!.charges.map((charge, index) =>
            index === 1
              ? {
                  ...charge,
                  tier: { ...charge.tier!, minimumInclusive: 1_001 },
                }
              : charge,
          ),
        },
      },
    }),
  );
});

test('known completion, embedding, and rerank fixtures reproduce exact cost', () => {
  const snapshot = prices();
  assert.equal(
    operationCost(snapshot, 'completion', {
      inputTokens: 1_000,
      cachedInputTokens: 500,
      outputTokens: 250,
      requests: 1,
    }),
    2.5,
  );
  assert.equal(
    operationCost(snapshot, 'embedding', {
      embeddingInputTokens: 250,
      requests: 1,
    }),
    0.05,
  );
  const rerank = operationCost(snapshot, 'rerank', {
    rerankInputTokens: 200,
    rerankSearchUnits: 2,
    documents: 5,
    requests: 1,
  });
  assert.ok(Math.abs(rerank - 0.12) < Number.EPSILON);
});

test('meter captures authenticated retrieval usage and partial failures', async () => {
  const captured = createUsageTransport(
    transport([
      response({
        data: [{ embedding: [0.1, 0.2] }],
        usage: { prompt_tokens: 250, total_tokens: 250 },
      }),
      response({
        results: [{ index: 0, relevance_score: 0.9 }],
        usage: { total_tokens: 200, search_units: 2 },
      }),
      response({ data: [], usage: { prompt_tokens: 10, total_tokens: 10 } }),
    ]),
  );
  const source = createOpenAiProvider({
    transport: captured.transport,
    apiKey: 'test',
    logger: pino({ level: 'silent' }),
  });
  const meter = createMeteredProvider(source, prices(), {
    retrievalUsage: captured.take,
  });
  await meter.provider.embedding({ model: 'embedding', input: 'text' });
  await meter.provider.rerank({
    model: 'rerank',
    query: 'query',
    documents: ['a', 'b', 'c', 'd', 'e'],
  });
  await assert.rejects(
    meter.provider.embedding({ model: 'embedding', input: 'broken' }),
  );
  const metered = meter.snapshot();
  assert.deepEqual(
    { ...metered, costUsd: 0 },
    {
      inputTokens: 0,
      outputTokens: 0,
      cachedInputTokens: 0,
      reasoningTokens: 0,
      embeddingInputTokens: 260,
      rerankInputTokens: 200,
      rerankDocuments: 5,
      rerankSearchUnits: 2,
      completionRequests: 0,
      embeddingRequests: 2,
      rerankRequests: 1,
      costUsd: 0,
    },
  );
  assert.ok(Math.abs(metered.costUsd - 0.172) < Number.EPSILON);
});

test('metering journal preserves allowlisted usage for recovery', async () => {
  const root = await mkdtemp(join(tmpdir(), 'mosaic-metering-'));
  try {
    const store = createMeteringStore(root);
    const scope = { id: 'run.test', attempt: 1 };
    await store.append(scope, {
      operation: 'embedding',
      stage: 'run',
      model: 'embedding',
      usage: {
        inputTokens: 0,
        outputTokens: 0,
        embeddingInputTokens: 7,
        rerankInputTokens: 0,
        rerankDocuments: 0,
        rerankSearchUnits: 0,
        completionRequests: 0,
        embeddingRequests: 1,
        rerankRequests: 0,
        costUsd: 0.7,
      },
      costUsd: 0.7,
    });
    const usage = await store.usage(scope);
    assert.equal(usage.embeddingInputTokens, 7);
    assert.equal(usage.embeddingRequests, 1);
    assert.equal(usage.costUsd, 0.7);
    assert.equal((await store.read(scope))[0]?.previousHash, null);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
