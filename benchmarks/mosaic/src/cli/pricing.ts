import { z } from 'zod';

import { artifactHash } from '../core/index.js';
import type { EngineUsage } from '../runtime/index.js';

const nonNegative = z.number().finite().nonnegative();

const ModelPriceV1 = z
  .object({
    inputPerMillion: nonNegative.default(0),
    outputPerMillion: nonNegative.default(0),
    cachedInputPerMillion: nonNegative.optional(),
    perRequest: nonNegative.default(0),
  })
  .strict();

export const PricesV1 = z
  .object({
    schemaVersion: z.literal(1),
    currency: z.literal('USD'),
    capturedAt: z.string().datetime({ offset: true }),
    source: z.string().url(),
    models: z.record(z.string().trim().min(1), ModelPriceV1),
  })
  .strict();

export type Prices = z.infer<typeof PricesV1>;
export type TokenUsage = Omit<EngineUsage, 'costUsd'>;

export const parsePrices = (value: unknown): Prices => PricesV1.parse(value);

export const pricesHash = (prices: Prices): string => artifactHash(prices);

export const tokenCost = (
  prices: Prices,
  model: string,
  usage: TokenUsage,
): number => {
  const price = prices.models[model];
  if (price === undefined)
    throw new TypeError(`missing frozen price for model: ${model}`);
  const cached = Math.min(usage.cachedInputTokens ?? 0, usage.inputTokens);
  const uncached = usage.inputTokens - cached;
  return (
    (uncached * price.inputPerMillion +
      cached * (price.cachedInputPerMillion ?? price.inputPerMillion) +
      usage.outputTokens * price.outputPerMillion) /
    1_000_000
  );
};

export const requestCost = (prices: Prices, model: string): number => {
  const price = prices.models[model];
  if (price === undefined)
    throw new TypeError(`missing frozen price for model: ${model}`);
  return price.perRequest;
};
