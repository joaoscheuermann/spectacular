import assert from 'node:assert/strict';
import test from 'node:test';

import {
  ConfigInputSchema,
  defaultConfig,
  type DoricConfig,
} from '../src/lib/config.js';
import { createConfigService } from '../src/lib/config-service.js';
import type { Generation } from '../src/lib/generation.js';

test('accepts the complete default and rejects unsafe or inconsistent config', () => {
  assert.equal(ConfigInputSchema.safeParse(defaultConfig).success, true);

  const secret = structuredClone(defaultConfig) as unknown as Record<
    string,
    unknown
  >;
  (secret.providers as Record<string, unknown>[])[0]!.apiKey = 'private';
  assert.equal(ConfigInputSchema.safeParse(secret).success, false);

  const invalid = structuredClone(defaultConfig);
  invalid.providers[0]!.apiKeyEnv = 'TOKEN';
  invalid.providers[0]!.baseUrl = 'file:///tmp/provider';
  invalid.models.execution.providerId = 'missing';
  invalid.routing.maxSkills = invalid.routing.maxRetrievedCandidates + 1;
  assert.equal(ConfigInputSchema.safeParse(invalid).success, false);
});

test('serializes replacements and preserves the active generation after failure', async () => {
  let revision = 1;
  const writes: string[] = [];
  const initial = snapshot(defaultConfig, revision);
  const store = {
    load: async () => initial,
    replace: async (configuration: typeof defaultConfig) => {
      writes.push(configuration.models.planning.model);
      return snapshot(configuration, ++revision);
    },
  };
  let releaseFirst: () => void = () => undefined;
  const firstGate = new Promise<void>((resolve) => {
    releaseFirst = resolve;
  });
  const build = async ({ snapshot: current }: { snapshot: DoricConfig }) => {
    const model = current.configuration.models.planning.model;
    if (model === 'first') await firstGate;
    if (model === 'broken') throw new Error('reindex failed');
    return { snapshot: current, marker: model } as unknown as Generation;
  };
  const service = await createConfigService({
    store,
    bundles: [],
    logger: {} as never,
    buildGeneration: build as never,
  });
  const first = configured('first');
  const second = configured('second');
  const firstWrite = service.replace(first);
  const secondWrite = service.replace(second);
  await Promise.resolve();
  assert.deepEqual(writes, []);
  releaseFirst();
  await Promise.all([firstWrite, secondWrite]);
  assert.deepEqual(writes, ['first', 'second']);
  assert.equal(
    service.current().snapshot.configuration.models.planning.model,
    'second',
  );

  await assert.rejects(
    service.replace(configured('broken')),
    /reindex failed/u,
  );
  assert.deepEqual(writes, ['first', 'second']);
  assert.equal(
    service.current().snapshot.configuration.models.planning.model,
    'second',
  );
});

const configured = (model: string) => {
  const config = structuredClone(defaultConfig);
  config.models.planning.model = model;
  return config;
};

const snapshot = (
  configuration: typeof defaultConfig,
  revision: number,
): DoricConfig => ({
  configuration,
  revision,
  updatedAt: new Date(revision * 1000).toISOString(),
});
