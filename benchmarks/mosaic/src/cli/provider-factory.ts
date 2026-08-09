import {
  createFetchTransport,
  createOpenAiProvider,
  type LlmProvider,
  type Model,
} from 'llms';
import type { Logger } from 'pino';

import type { Prices } from './pricing.js';
import { createMeteredProvider, type MeteredProvider } from './provider.js';
import { createUsageTransport } from './usage-transport.js';

const OPENROUTER_BASE_URL = 'https://openrouter.ai/api/v1';

const models = async (
  transport: ReturnType<typeof createFetchTransport>,
  apiKey: string,
  signal?: AbortSignal,
): Promise<readonly Model[]> => {
  const response = await transport.request({
    method: 'GET',
    url: `${OPENROUTER_BASE_URL}/models?output_modalities=all`,
    headers: {
      authorization: `Bearer ${apiKey}`,
      accept: 'application/json',
    },
    signal,
  });
  if (response.status >= 400) {
    throw new Error('OpenRouter model catalog request failed');
  }
  const parsed = JSON.parse(response.body) as { readonly data?: unknown };
  if (!Array.isArray(parsed.data)) {
    throw new TypeError('OpenRouter model catalog is invalid');
  }
  return parsed.data.flatMap((raw) => {
    if (typeof raw !== 'object' || raw === null || Array.isArray(raw))
      return [];
    const id = (raw as Readonly<Record<string, unknown>>)['id'];
    return typeof id === 'string' && id.trim().length > 0
      ? [{ id, name: id, provider: 'openrouter', raw }]
      : [];
  });
};

/** Composes the benchmark provider with retrieval-usage interception. */
export const createStudyProvider = (
  apiKey: string,
  prices: Prices,
  logger: Logger,
): MeteredProvider => {
  const fetchTransport = createFetchTransport();
  const usage = createUsageTransport(fetchTransport);
  const compatible = createOpenAiProvider({
    transport: usage.transport,
    baseUrl: OPENROUTER_BASE_URL,
    apiKey,
    logger,
  });
  const source: LlmProvider = {
    ...compatible,
    models: (signal) => models(fetchTransport, apiKey, signal),
    async validateModel(model, signal) {
      const found = (await models(fetchTransport, apiKey, signal)).find(
        ({ id }) => id === model,
      );
      if (found === undefined)
        throw new TypeError(`model is unavailable: ${model}`);
      return found;
    },
  };
  const meter = createMeteredProvider(source, prices, {
    retrievalUsage: usage.take,
    retrievalCaptured: true,
  });
  usage.bind(meter.recordRetrieval);
  return meter;
};
