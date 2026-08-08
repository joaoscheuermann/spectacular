import assert from 'node:assert/strict';
import test from 'node:test';

import type { Skill } from 'bundle';
import type { Tool } from 'tool';

import { mosaic, type MosaicOptions } from '../src/index.js';

test('preserves both search retrievers at the factory boundary', () => {
  const options = validOptions();
  assert.doesNotThrow(() => mosaic(options));
  assert.ok(options.skills.retriever);
  assert.ok(options.tools.retriever);
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

test('requires searchable skill and tool retrievers', () => {
  const invalidSkillRetriever = validOptions();
  invalidSkillRetriever.skills.retriever = {} as never;
  assert.throws(
    () => mosaic(invalidSkillRetriever),
    /skills\.retriever must provide search/u,
  );

  const invalidToolRetriever = validOptions();
  invalidToolRetriever.tools.retriever = {} as never;
  assert.throws(
    () => mosaic(invalidToolRetriever),
    /tools\.retriever must provide search/u,
  );
});

test('requires an explicit non-negative integer localized revision limit', () => {
  for (const value of [-1, 0.5, Number.NaN]) {
    const options = validOptions();
    options.revision = { max: value };
    assert.throws(
      () => mosaic(options),
      /revision\.max must be an integer >= 0/u,
    );
  }

  const zero = validOptions();
  zero.revision = { max: 0 };
  assert.doesNotThrow(() => mosaic(zero));

  const missing = validOptions() as unknown as {
    revision?: { max: number };
  };
  delete missing.revision;
  assert.throws(() => mosaic(missing as MosaicOptions), /revision|max/u);
});

const validOptions = (): MutableOptions => ({
  logger: {} as never,
  provider: {} as never,
  models: {
    default: 'default-model',
    reranker: 'reranker-model',
  },
  routing: { maxCandidates: 5, maxSkills: 5 },
  revision: { max: 3 },
  skills: { required: [], menu: [], retriever: { search: async () => [] } },
  tools: { required: [], menu: [], retriever: { search: async () => [] } },
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
