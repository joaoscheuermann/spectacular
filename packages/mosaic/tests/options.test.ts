import assert from 'node:assert/strict';
import test from 'node:test';

import type { Skill } from 'bundle';
import type { Tool } from 'tool';

import { mosaic, type MosaicOptions } from '../src/index.js';
import { mosaicProviders } from './structured.js';

test('preserves both search retrievers at the factory boundary', () => {
  const options = validOptions();
  assert.doesNotThrow(() => mosaic(options));
  assert.ok(options.skills.retriever);
  assert.ok(options.tools.retriever);
});

test('rejects invalid routing limits duplicate catalogs and invalid required tools', () => {
  const invalidLimits = validOptions();
  invalidLimits.routing = {
    maxHintCandidates: 1,
    maxRetrievedCandidates: 1,
    maxSkills: 2,
  };
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

test('requires both independent routing limits and rejects the legacy alias', () => {
  for (const field of [
    'maxHintCandidates',
    'maxRetrievedCandidates',
  ] as const) {
    for (const value of [0, 0.5, Number.NaN, 2 ** 53]) {
      const options = validOptions();
      options.routing[field] = value;
      assert.throws(() => mosaic(options), new RegExp(field, 'u'));
    }
  }

  const legacy = validOptions() as unknown as MutableOptions & {
    routing: { maxCandidates?: number };
  };
  const legacyRouting = legacy.routing as unknown as Record<string, unknown>;
  delete legacyRouting['maxHintCandidates'];
  delete legacyRouting['maxRetrievedCandidates'];
  legacyRouting['maxCandidates'] = 5;
  assert.throws(() => mosaic(legacy as MosaicOptions), /maxHintCandidates/u);
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

test('requires a positive safe integer node turn limit', () => {
  for (const value of [0, -1, 0.5, Number.NaN, Infinity, 2 ** 53]) {
    const options = validOptions();
    options.execution = { maxTurns: value };
    assert.throws(
      () => mosaic(options),
      /execution\.maxTurns must be an integer >= 1/u,
    );
  }

  const missing = validOptions() as unknown as {
    execution?: { maxTurns: number };
  };
  delete missing.execution;
  assert.throws(
    () => mosaic(missing as MosaicOptions),
    /execution\.maxTurns must be an integer >= 1/u,
  );
});

test('requires a model and reasoning effort for every model-backed stage', () => {
  for (const stage of ['planning', 'revision', 'execution'] as const) {
    const missingModel = validOptions();
    missingModel.models[stage] = { model: ' ', effort: 'low' };
    assert.throws(
      () => mosaic(missingModel),
      new RegExp(`${stage}\\.model`, 'u'),
    );

    const invalidEffort = validOptions();
    invalidEffort.models[stage] = {
      model: 'model',
      effort: 'invalid' as never,
    };
    assert.throws(
      () => mosaic(invalidEffort),
      new RegExp(`${stage}\\.effort`, 'u'),
    );
  }

  for (const stage of ['reranker', 'embedder'] as const) {
    const missingModel = validOptions();
    missingModel.models[stage] = ' ';
    assert.throws(() => mosaic(missingModel), new RegExp(stage, 'u'));
  }
});

const validOptions = (): MutableOptions => ({
  logger: {} as never,
  providers: mosaicProviders({} as never),
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
  indexText: `${name} | ${name} description | ${allowedTools.join(',')} | ${name} body`,
});

const tool = (name: string): Tool => ({
  name,
  input: {} as Tool['input'],
  output: {} as Tool['output'],
  definition: { name, inputSchema: {}, outputSchema: {} },
  execute: async () => undefined,
});
