import { z } from 'zod';
import { ProviderErrorObject } from 'llms';
import type { MosaicOptions } from 'mosaic';

import type { EngineUsage } from '../runtime/index.js';
import type {
  BaselineDecision,
  BaselineModel,
  BaselinePlan,
  ModelAnswer,
  PlannedGoal,
} from './baselines.js';

const goal = z
  .object({
    id: z.string().trim().min(1),
    goal: z.string().trim().min(1),
    doneWhen: z.array(z.string().trim().min(1)).min(1),
  })
  .strict();
const plan = z.object({ goals: z.array(goal).min(1) }).strict();
const decision = z
  .object({
    status: z.enum(['completed', 'failed', 'needs_revision']),
    output: z.string(),
    toolCalls: z.array(
      z.object({ name: z.string().trim().min(1), input: z.json() }).strict(),
    ),
    revisionReason: z.string().trim().min(1).optional(),
  })
  .strict();

type Provider = MosaicOptions['provider'];
type TokenUsage = Omit<EngineUsage, 'costUsd'>;

export interface ProviderBaselineOptions {
  readonly provider: Provider;
  readonly model: string;
  readonly cost?: (usage: TokenUsage) => number;
  readonly onUsage?: (usage: EngineUsage) => void | Promise<void>;
}

/** Authenticates the public provider error wrapper without inspecting unsafe text. */
export const isProviderError = (error: unknown): error is ProviderErrorObject =>
  error instanceof ProviderErrorObject;

/** Matches an authenticated provider failure by its stable public code only. */
export const isProviderErrorCode = (
  error: unknown,
  code: string,
): error is ProviderErrorObject =>
  isProviderError(error) && error.data.code === code;

const usage = (
  value:
    | {
        readonly inputTokens?: number;
        readonly outputTokens?: number;
        readonly cachedInputTokens?: number;
        readonly reasoningTokens?: number;
      }
    | undefined,
  cost: ProviderBaselineOptions['cost'],
): EngineUsage => {
  const tokens = {
    inputTokens: value?.inputTokens ?? 0,
    outputTokens: value?.outputTokens ?? 0,
    cachedInputTokens: value?.cachedInputTokens,
    reasoningTokens: value?.reasoningTokens,
  };
  return { ...tokens, costUsd: cost?.(tokens) ?? 0 };
};

const complete = async <Value>(
  options: ProviderBaselineOptions,
  schema: z.ZodType<Value>,
  request: { readonly system: string; readonly user: string },
): Promise<ModelAnswer<Value>> => {
  const result = await options.provider.complete({
    model: options.model,
    effort: 'medium',
    messages: [
      { role: 'system', content: request.system },
      { role: 'user', content: request.user },
    ],
    schema,
    flags: { sensitiveOutput: true },
  });
  const metered = usage(result.usage, options.cost);
  await options.onUsage?.(metered);
  return { value: result.structured, usage: metered };
};

/** Adapts the provider-neutral structured API to the frozen baseline model contract. */
export const createProviderBaselineModel = (
  options: ProviderBaselineOptions,
): BaselineModel => ({
  plan: (request): Promise<ModelAnswer<BaselinePlan>> =>
    complete(options, plan, request),
  execute: (request): Promise<ModelAnswer<BaselineDecision>> =>
    complete(options, decision, request),
  revise: (request): Promise<ModelAnswer<PlannedGoal>> =>
    complete(options, goal, request),
});
