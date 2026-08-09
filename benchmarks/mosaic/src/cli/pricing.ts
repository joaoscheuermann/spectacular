import { z } from 'zod';

import { artifactHash } from '../core/index.js';

const nonNegativeInteger = z.number().int().safe().nonnegative();
const positiveInteger = z.number().int().safe().positive();
const positivePrice = z.number().finite().positive();

export const PriceUnitV1 = z.enum([
  'input-token',
  'cached-input-token',
  'output-token',
  'embedding-input-token',
  'rerank-input-token',
  'rerank-search-unit',
  'request',
]);

const PriceTierV1 = z
  .object({
    id: z.string().trim().min(1),
    basis: z.enum(['input-tokens', 'total-tokens', 'documents']),
    minimumInclusive: nonNegativeInteger,
    maximumExclusive: positiveInteger.optional(),
  })
  .strict()
  .refine(
    ({ minimumInclusive, maximumExclusive }) =>
      maximumExclusive === undefined || maximumExclusive > minimumInclusive,
    { message: 'price tier maximum must exceed its minimum' },
  );

const PriceChargeV1 = z
  .object({
    unit: PriceUnitV1,
    quantity: positiveInteger,
    priceUsd: positivePrice,
    tier: PriceTierV1.optional(),
  })
  .strict();

const unitsForKind = {
  completion: new Set([
    'input-token',
    'cached-input-token',
    'output-token',
    'request',
  ]),
  embedding: new Set(['embedding-input-token', 'request']),
  rerank: new Set(['rerank-input-token', 'rerank-search-unit', 'request']),
} as const;

const ModelPriceV1 = z
  .object({
    kind: z.enum(['completion', 'embedding', 'rerank']),
    capturedAt: z.string().datetime({ offset: true }),
    source: z.string().url(),
    charges: z.array(PriceChargeV1).min(1),
  })
  .strict()
  .superRefine((model, context) => {
    const allowed = unitsForKind[model.kind];
    const groups = new Map<string, typeof model.charges>();
    model.charges.forEach((charge, index) => {
      if (!allowed.has(charge.unit as never)) {
        context.addIssue({
          code: 'custom',
          message: `${charge.unit} is not applicable to ${model.kind}`,
          path: ['charges', index, 'unit'],
        });
      }
      groups.set(charge.unit, [...(groups.get(charge.unit) ?? []), charge]);
    });
    for (const [unit, charges] of groups) {
      const tiered = charges.some(({ tier }) => tier !== undefined);
      if (!tiered && charges.length > 1) {
        context.addIssue({
          code: 'custom',
          message: `duplicate untiered charge for ${unit}`,
          path: ['charges'],
        });
        continue;
      }
      if (!tiered) continue;
      if (charges.some(({ tier }) => tier === undefined)) {
        context.addIssue({
          code: 'custom',
          message: `every ${unit} charge must declare its tier`,
          path: ['charges'],
        });
        continue;
      }
      const tiers = charges
        .map(({ tier }) => tier!)
        .sort((left, right) => left.minimumInclusive - right.minimumInclusive);
      const basis = tiers[0]?.basis;
      const complete = tiers.every(
        (tier, index) =>
          tier.basis === basis &&
          tier.minimumInclusive ===
            (index === 0 ? 0 : tiers[index - 1]?.maximumExclusive) &&
          (index < tiers.length - 1
            ? tier.maximumExclusive !== undefined
            : tier.maximumExclusive === undefined),
      );
      if (!complete) {
        context.addIssue({
          code: 'custom',
          message: `${unit} tiers must be contiguous from zero through infinity`,
          path: ['charges'],
        });
      }
    }
  });

export const PricesV1 = z
  .object({
    schemaVersion: z.literal(1),
    currency: z.literal('USD'),
    models: z.record(z.string().trim().min(1), ModelPriceV1),
  })
  .strict();

export type Prices = z.infer<typeof PricesV1>;
export type PriceUnit = z.infer<typeof PriceUnitV1>;
export type ModelPriceKind = Prices['models'][string]['kind'];

export interface BillableUsage {
  readonly inputTokens?: number;
  readonly outputTokens?: number;
  readonly cachedInputTokens?: number;
  readonly embeddingInputTokens?: number;
  readonly rerankInputTokens?: number;
  readonly rerankSearchUnits?: number;
  readonly requests?: number;
  readonly documents?: number;
}

export interface PriceRequirement {
  readonly model: string;
  readonly kind: ModelPriceKind;
}

export const parsePrices = (value: unknown): Prices => PricesV1.parse(value);
export const pricesHash = (prices: Prices): string => artifactHash(prices);

const safeCount = (value: number | undefined, name: string): number => {
  const count = value ?? 0;
  if (!Number.isSafeInteger(count) || count < 0) {
    throw new TypeError(`${name} must be a non-negative safe integer`);
  }
  return count;
};

const tierBasis = (
  basis: NonNullable<
    Prices['models'][string]['charges'][number]['tier']
  >['basis'],
  usage: BillableUsage,
): number => {
  if (basis === 'input-tokens') {
    return safeCount(
      (usage.inputTokens ?? 0) +
        (usage.embeddingInputTokens ?? 0) +
        (usage.rerankInputTokens ?? 0),
      'tier input tokens',
    );
  }
  if (basis === 'total-tokens') {
    return safeCount(
      (usage.inputTokens ?? 0) +
        (usage.outputTokens ?? 0) +
        (usage.embeddingInputTokens ?? 0) +
        (usage.rerankInputTokens ?? 0),
      'tier total tokens',
    );
  }
  return safeCount(usage.documents, 'tier documents');
};

const unitQuantity = (
  unit: PriceUnit,
  usage: BillableUsage,
  hasCachedRate: boolean,
): number => {
  const input = safeCount(usage.inputTokens, 'input tokens');
  const cached = Math.min(
    safeCount(usage.cachedInputTokens, 'cached input tokens'),
    input,
  );
  switch (unit) {
    case 'input-token':
      return hasCachedRate ? input - cached : input;
    case 'cached-input-token':
      return cached;
    case 'output-token':
      return safeCount(usage.outputTokens, 'output tokens');
    case 'embedding-input-token':
      return safeCount(usage.embeddingInputTokens, 'embedding input tokens');
    case 'rerank-input-token':
      return safeCount(usage.rerankInputTokens, 'rerank input tokens');
    case 'rerank-search-unit':
      return safeCount(usage.rerankSearchUnits, 'rerank search units');
    case 'request':
      return safeCount(usage.requests, 'requests');
  }
};

const applies = (
  tier: Prices['models'][string]['charges'][number]['tier'],
  usage: BillableUsage,
): boolean => {
  if (tier === undefined) return true;
  const value = tierBasis(tier.basis, usage);
  return (
    value >= tier.minimumInclusive &&
    (tier.maximumExclusive === undefined || value < tier.maximumExclusive)
  );
};

/** Calculates one request from its authenticated usage and exact price units. */
export const operationCost = (
  prices: Prices,
  model: string,
  usage: BillableUsage,
): number => {
  const price = prices.models[model];
  if (price === undefined) {
    throw new TypeError(`missing frozen price for model: ${model}`);
  }
  const hasCachedRate = price.charges.some(
    ({ unit }) => unit === 'cached-input-token',
  );
  return price.charges.reduce((total, charge) => {
    if (!applies(charge.tier, usage)) return total;
    return (
      total +
      (unitQuantity(charge.unit, usage, hasCachedRate) * charge.priceUsd) /
        charge.quantity
    );
  }, 0);
};

/** Ensures every active study role has a typed, positive, applicable price. */
export const validatePriceCoverage = (
  prices: Prices,
  requirements: readonly PriceRequirement[],
): void => {
  for (const requirement of requirements) {
    const price = prices.models[requirement.model];
    if (price === undefined) {
      throw new TypeError(
        `missing frozen price for model: ${requirement.model}`,
      );
    }
    if (price.kind !== requirement.kind) {
      throw new TypeError(
        `frozen price kind mismatch for model: ${requirement.model}`,
      );
    }
  }
};

export const tokenCost = (
  prices: Prices,
  model: string,
  usage: BillableUsage,
): number => operationCost(prices, model, { ...usage, requests: 1 });
