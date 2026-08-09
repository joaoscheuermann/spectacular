import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import type { LlmProvider } from 'llms';

import {
  buildProductionIndex,
  IndexAbruptInterruption,
  loadProductionIndex,
  productionIndexRecipe,
  type IndexCrashBoundary,
} from '../src/cli/index-lifecycle.js';
import { createMeteringStore } from '../src/cli/metering-store.js';
import { parsePrices } from '../src/cli/pricing.js';
import { createMeteredProvider } from '../src/cli/provider.js';
import type { RetrievalUsage } from '../src/cli/usage-transport.js';

const values = [
  {
    name: 'skill-one',
    description: 'First skill.',
    allowedTools: ['read'],
    indexText: '# Skill one\n\nFirst body.',
  },
  {
    name: 'skill-two',
    description: 'Second skill.',
    allowedTools: ['write'],
    indexText: '# Skill two\n\nSecond body.',
  },
] as const;

const prices = parsePrices({
  schemaVersion: 1,
  currency: 'USD',
  models: {
    embed: {
      kind: 'embedding',
      capturedAt: '2026-08-09T00:00:00.000Z',
      source: 'https://example.test/embed',
      charges: [
        { unit: 'embedding-input-token', quantity: 1_000, priceUsd: 1 },
      ],
    },
  },
});

const fixture = () => {
  let calls = 0;
  const usage: RetrievalUsage[] = [];
  const source = {
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
    complete: async () => {
      throw new Error('unused');
    },
    stream: async function* () {
      return;
    },
    embedding: async () => {
      calls += 1;
      usage.push({
        operation: 'embedding',
        model: 'embed',
        inputTokens: 1,
        searchUnits: 0,
        documents: 0,
      });
      return [calls, -calls];
    },
    rerank: async () => {
      throw new Error('unused');
    },
    models: async () => [],
    validateModel: async () => {
      throw new Error('unused');
    },
  } as unknown as LlmProvider;
  const meter = createMeteredProvider(source, prices, {
    retrievalUsage: (operation, model) => {
      const index = usage.findIndex(
        (entry) => entry.operation === operation && entry.model === model,
      );
      return index < 0 ? undefined : usage.splice(index, 1)[0];
    },
  });
  return { meter, calls: () => calls };
};

for (const boundary of [
  'after-reservation',
  'after-entry',
  'before-artifact',
  'after-artifact',
  'before-terminal',
] as const satisfies readonly IndexCrashBoundary[]) {
  test(`index construction resumes without repeating paid entries after ${boundary}`, async () => {
    const root = await mkdtemp(join(tmpdir(), 'mosaic-index-'));
    try {
      const current = fixture();
      let armed = true;
      await assert.rejects(
        buildProductionIndex({
          root,
          provider: current.meter.provider,
          meter: current.meter,
          metering: createMeteringStore(root),
          values,
          embedder: 'embed',
          dimensions: 2,
          crash: (value) => {
            if (armed && value === boundary) {
              armed = false;
              throw new IndexAbruptInterruption();
            }
          },
        }),
        IndexAbruptInterruption,
      );
      const resumed = await buildProductionIndex({
        root,
        provider: current.meter.provider,
        meter: current.meter,
        metering: createMeteringStore(root),
        values,
        embedder: 'embed',
        dimensions: 2,
      });
      assert.equal(current.calls(), 4);
      assert.equal(resumed.artifact.entries.length, 4);
      const reused = await buildProductionIndex({
        root,
        provider: current.meter.provider,
        meter: current.meter,
        metering: createMeteringStore(root),
        values,
        embedder: 'embed',
        dimensions: 2,
      });
      assert.equal(reused.reused, true);
      assert.equal(reused.artifact.indexHash, resumed.artifact.indexHash);
      assert.equal(current.calls(), 4);
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });
}

test('index load rejects changed corpus, model, dimension, and artifact bytes', async () => {
  const root = await mkdtemp(join(tmpdir(), 'mosaic-index-integrity-'));
  try {
    const current = fixture();
    const built = await buildProductionIndex({
      root,
      provider: current.meter.provider,
      meter: current.meter,
      metering: createMeteringStore(root),
      values,
      embedder: 'embed',
      dimensions: 2,
    });
    const changed = values.map((value, index) =>
      index === 0 ? { ...value, indexText: `${value.indexText}!` } : value,
    );
    await assert.rejects(
      loadProductionIndex(
        built.path,
        productionIndexRecipe(changed, 'embed', 2),
      ),
      /recipe mismatch/u,
    );
    await assert.rejects(
      loadProductionIndex(
        built.path,
        productionIndexRecipe(values, 'other-embedder', 2),
      ),
      /recipe mismatch/u,
    );
    await assert.rejects(
      loadProductionIndex(
        built.path,
        productionIndexRecipe(values, 'embed', 3),
      ),
      /recipe mismatch/u,
    );
    const artifact = JSON.parse(await readFile(built.path, 'utf8')) as {
      entries: { vector: number[] }[];
    };
    artifact.entries[0]!.vector[0] = 999;
    await writeFile(built.path, `${JSON.stringify(artifact)}\n`, 'utf8');
    await assert.rejects(loadProductionIndex(built.path), /hash or recipe/u);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('resume never repeats an index call whose paid boundary is unresolved', async () => {
  const root = await mkdtemp(join(tmpdir(), 'mosaic-index-unresolved-'));
  try {
    const current = fixture();
    await assert.rejects(
      buildProductionIndex({
        root,
        provider: current.meter.provider,
        meter: current.meter,
        metering: createMeteringStore(root),
        values,
        embedder: 'embed',
        dimensions: 2,
        crash: (boundary) => {
          if (boundary === 'after-entry-reservation') {
            throw new IndexAbruptInterruption();
          }
        },
      }),
      IndexAbruptInterruption,
    );
    assert.equal(current.calls(), 0);
    await assert.rejects(
      buildProductionIndex({
        root,
        provider: current.meter.provider,
        meter: current.meter,
        metering: createMeteringStore(root),
        values,
        embedder: 'embed',
        dimensions: 2,
      }),
      /audited recovery/u,
    );
    assert.equal(current.calls(), 0);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
