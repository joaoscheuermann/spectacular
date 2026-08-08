import type {
  LlmProvider,
  ProviderFinished,
  ProviderRequest,
  ProviderStreamEvent,
  UsageMetadata,
} from 'llms';

import type { EngineUsage } from '../runtime/index.js';
import type { Prices, TokenUsage } from './pricing.js';
import { requestCost, tokenCost } from './pricing.js';

export interface MeteredProvider {
  readonly provider: LlmProvider;
  readonly reset: () => void;
  readonly snapshot: () => EngineUsage;
  readonly costFor: (model: string, usage: TokenUsage) => number;
}

const tokenUsage = (usage: UsageMetadata | undefined): TokenUsage => ({
  inputTokens: usage?.inputTokens ?? 0,
  outputTokens: usage?.outputTokens ?? 0,
  cachedInputTokens: usage?.cachedInputTokens,
  reasoningTokens: usage?.reasoningTokens,
});

const mediumRequest = <Output>(
  request: ProviderRequest<Output>,
): ProviderRequest<Output> => ({
  ...request,
  effort: 'medium',
  flags: { ...request.flags, includeUsage: true },
});

/** Forces the frozen effort and meters only allowlisted usage fields. */
export const createMeteredProvider = (
  source: LlmProvider,
  prices: Prices,
): MeteredProvider => {
  let current: EngineUsage = {
    inputTokens: 0,
    outputTokens: 0,
    cachedInputTokens: 0,
    reasoningTokens: 0,
    costUsd: 0,
  };

  const addTokens = (
    model: string,
    metadata: UsageMetadata | undefined,
  ): void => {
    const usage = tokenUsage(metadata);
    current = {
      inputTokens: current.inputTokens + usage.inputTokens,
      outputTokens: current.outputTokens + usage.outputTokens,
      cachedInputTokens:
        (current.cachedInputTokens ?? 0) + (usage.cachedInputTokens ?? 0),
      reasoningTokens:
        (current.reasoningTokens ?? 0) + (usage.reasoningTokens ?? 0),
      costUsd: current.costUsd + tokenCost(prices, model, usage),
    };
  };
  const addRequest = (model: string): void => {
    current = {
      ...current,
      costUsd: current.costUsd + requestCost(prices, model),
    };
  };

  const complete = (async (request: ProviderRequest) => {
    const result = await source.complete(mediumRequest(request) as never);
    addTokens(request.model, result.usage);
    addRequest(request.model);
    return result;
  }) as LlmProvider['complete'];

  const stream = ((request: ProviderRequest) => {
    const events = source.stream(mediumRequest(request) as never);
    return (async function* (): AsyncIterable<ProviderStreamEvent<unknown>> {
      for await (const event of events) {
        if (event.type === 'response.finished') {
          addTokens(request.model, event.finish.usage);
          addRequest(request.model);
        }
        yield event;
      }
    })();
  }) as LlmProvider['stream'];

  const provider: LlmProvider = {
    metadata: source.metadata,
    capabilities: source.capabilities,
    complete,
    stream,
    async embedding(request) {
      const result = await source.embedding(request);
      addRequest(request.model);
      return result;
    },
    async rerank(request) {
      const result = await source.rerank(request);
      addRequest(request.model);
      return result;
    },
    models: (signal) => source.models(signal),
    validateModel: (model, signal) => source.validateModel(model, signal),
  };

  return {
    provider,
    reset: () => {
      current = {
        inputTokens: 0,
        outputTokens: 0,
        cachedInputTokens: 0,
        reasoningTokens: 0,
        costUsd: 0,
      };
    },
    snapshot: () => ({ ...current }),
    costFor: (model, usage) =>
      tokenCost(prices, model, usage) + requestCost(prices, model),
  };
};

export const finishedUsage = (result: ProviderFinished): TokenUsage =>
  tokenUsage(result.usage);
