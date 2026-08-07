import assert from 'node:assert/strict';
import test from 'node:test';

import type { Skill } from 'bundle';
import type { Tool } from 'tool';

import { mosaic, type MosaicOptions } from '../src/index.js';

test('preserves the embedder and both vector indexes at the factory boundary', () => {
  const options = validOptions();
  assert.doesNotThrow(() => mosaic(options));
  assert.equal(options.models.embedder, 'embedder-model');
  assert.ok(options.skills.embeddings);
  assert.ok(options.tools.embeddings);
});

test('rejects invalid routing limits duplicate catalogs and invalid required tools', () => {
  const invalidLimits = validOptions();
  invalidLimits.routing = { maxCandidates: 1, maxSkills: 2 };
  assert.throws(() => mosaic(invalidLimits), /maxSkills must not exceed/u);

  const duplicate = validOptions();
  duplicate.skills.menu = [skill('selected'), skill('selected')];
  assert.throws(() => mosaic(duplicate), /duplicate selected/u);

  const nonBase = validOptions();
  nonBase.skills.required = [skill('required', ['special'])];
  nonBase.skills.menu = [...nonBase.skills.required];
  nonBase.tools.menu = [tool('special')];
  assert.throws(() => mosaic(nonBase), /non-base tool special/u);
});

const validOptions = (): MutableOptions => ({
  logger: {} as never,
  provider: {} as never,
  models: {
    default: 'default-model',
    reranker: 'reranker-model',
    embedder: 'embedder-model',
  },
  routing: { maxCandidates: 5, maxSkills: 5 },
  skills: { required: [], menu: [], embeddings: {} as never },
  tools: { required: [], menu: [], embeddings: {} as never },
});

type MutableOptions = {
  -readonly [Key in keyof MosaicOptions]: MosaicOptions[Key] extends object
    ? {
        -readonly [Nested in keyof MosaicOptions[Key]]: MosaicOptions[Key][Nested];
      }
    : MosaicOptions[Key];
};

const skill = (name: string, allowedTools: readonly string[] = []): Skill => ({
  name,
  description: `${name} description`,
  body: `${name} body`,
  allowedTools: [...allowedTools],
});

const tool = (name: string): Tool => ({
  name,
  schema: {} as Tool['schema'],
  definition: { name, inputSchema: {} },
  execute: async () => undefined,
});
