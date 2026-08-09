import type { LlmProvider, Model } from 'llms';

import {
  EMBEDDING_MODEL,
  FIRST_REPLICATION_CANDIDATE,
  PRIMARY_MODEL,
  RERANKER_MODEL,
} from '../config/index.js';

export interface CapabilityReport {
  readonly modelIds: readonly string[];
  readonly paidProbesRequired: readonly ['embedding', 'rerank'];
}

const record = (
  value: unknown,
): Readonly<Record<string, unknown>> | undefined =>
  typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as Readonly<Record<string, unknown>>)
    : undefined;

const strings = (value: unknown): readonly string[] =>
  Array.isArray(value)
    ? value.filter((entry): entry is string => typeof entry === 'string')
    : [];

const parameters = (model: Model): ReadonlySet<string> =>
  new Set(strings(record(model.raw)?.['supported_parameters']));

const hasAny = (
  values: ReadonlySet<string>,
  required: readonly string[],
): boolean => required.some((value) => values.has(value));

const requireCompletionCapabilities = (model: Model): void => {
  const supported = parameters(model);
  if (!hasAny(supported, ['tools', 'tool_choice'])) {
    throw new TypeError(`model metadata does not verify tools: ${model.id}`);
  }
  if (!hasAny(supported, ['structured_outputs', 'response_format'])) {
    throw new TypeError(
      `model metadata does not verify structured output: ${model.id}`,
    );
  }
  if (!hasAny(supported, ['reasoning', 'reasoning_effort'])) {
    throw new TypeError(
      `model metadata does not verify medium effort: ${model.id}`,
    );
  }
};

const modelIds = (candidateModel: string): readonly string[] => [
  PRIMARY_MODEL.model,
  candidateModel,
  EMBEDDING_MODEL.model,
  RERANKER_MODEL,
];

/** Validates every capability metadata can prove without a paid request. */
export const validateFreeCapabilities = async (
  provider: LlmProvider,
  candidateModel: string = FIRST_REPLICATION_CANDIDATE.model,
): Promise<CapabilityReport> => {
  if (
    !provider.capabilities.tools ||
    !provider.capabilities.structuredOutputs ||
    !provider.capabilities.reasoning ||
    !provider.capabilities.embeddings ||
    !provider.capabilities.reranking ||
    !provider.capabilities.modelListing
  ) {
    throw new TypeError('provider metadata lacks a required study capability');
  }
  const available = new Map(
    (await provider.models()).map((model) => [model.id, model]),
  );
  const required = modelIds(candidateModel);
  for (const id of required) {
    if (!available.has(id)) throw new TypeError(`model is unavailable: ${id}`);
  }
  requireCompletionCapabilities(available.get(PRIMARY_MODEL.model)!);
  requireCompletionCapabilities(available.get(candidateModel)!);
  return {
    modelIds: required,
    // Dimensions, endpoint result shape, and authenticated usage are not
    // provable from the model catalog and therefore remain explicit probes.
    paidProbesRequired: ['embedding', 'rerank'],
  };
};

/** Runs the two explicit paid probes whose properties metadata cannot prove. */
export const paidCapabilityProbes = (
  provider: LlmProvider,
): {
  readonly embedding: () => Promise<void>;
  readonly rerank: () => Promise<void>;
} => ({
  embedding: async () => {
    const vector = await provider.embedding({
      model: EMBEDDING_MODEL.model,
      input: 'MOSAIC metering capability probe.',
      dimensions: EMBEDDING_MODEL.dimensions,
      flags: { sensitiveOutput: true },
    });
    if (vector.length !== EMBEDDING_MODEL.dimensions) {
      throw new TypeError('embedding probe returned the wrong dimensions');
    }
  },
  rerank: async () => {
    const result = await provider.rerank({
      model: RERANKER_MODEL,
      query: 'MOSAIC capability probe',
      documents: ['MOSAIC capability probe'],
      topN: 1,
      flags: { sensitiveOutput: true },
    });
    if (result.length !== 1 || result[0]?.index !== 0) {
      throw new TypeError('rerank probe returned an incompatible result');
    }
  },
});
