import { z } from 'zod';
import { AgentErrorObject, createAgent } from 'agent';
import { ProviderErrorObject } from 'llms';
import type { MosaicOptions } from 'mosaic';
import { createMessageStorage } from 'messages';
import { createToolStorage } from 'tool';

import type { EngineUsage } from '../runtime/index.js';
import type {
  BaselineDecision,
  BaselineCallLifecycle,
  BaselineModel,
  BaselinePlan,
  ModelAnswer,
  PlannedGoal,
} from './baseline-contracts.js';

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

/** Matches an authenticated Agent lifecycle failure by its stable code. */
export const isAgentErrorCode = (
  error: unknown,
  code: string,
): error is AgentErrorObject =>
  error instanceof AgentErrorObject && error.data.code === code;

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

const optionalSum = (
  left: number | undefined,
  right: number | undefined,
): number | undefined =>
  left === undefined && right === undefined
    ? undefined
    : (left ?? 0) + (right ?? 0);

const addUsage = (left: EngineUsage, right: EngineUsage): EngineUsage => ({
  inputTokens: left.inputTokens + right.inputTokens,
  outputTokens: left.outputTokens + right.outputTokens,
  cachedInputTokens: optionalSum(
    left.cachedInputTokens,
    right.cachedInputTokens,
  ),
  reasoningTokens: optionalSum(left.reasoningTokens, right.reasoningTokens),
  costUsd: left.costUsd + right.costUsd,
});

const noLifecycle: BaselineCallLifecycle = {
  started: async () => undefined,
  succeeded: async () => undefined,
  failed: async () => undefined,
  structured: async () => undefined,
  repair: async () => undefined,
};

const complete = async <Value>(
  options: ProviderBaselineOptions,
  schema: z.ZodType<Value>,
  request: {
    readonly system: string;
    readonly user: string;
    readonly controls?: {
      readonly maxTurns: number;
      readonly structuredRepairRetries: number;
      readonly lifecycle: BaselineCallLifecycle;
    };
  },
): Promise<ModelAnswer<Value>> => {
  const controls = request.controls ?? {
    maxTurns: 16,
    structuredRepairRetries: 2,
    lifecycle: noLifecycle,
  };
  let metered = usage(undefined, options.cost);
  const provider = new Proxy(options.provider, {
    get(target, property, receiver) {
      if (property !== 'complete') {
        const value = Reflect.get(target, property, receiver) as unknown;
        return typeof value === 'function' ? value.bind(target) : value;
      }
      return async (providerRequest: Parameters<typeof target.complete>[0]) => {
        await controls.lifecycle.started({
          messageCount: providerRequest.messages.length,
          toolNames: providerRequest.tools?.map(({ name }) => name) ?? [],
        });
        let result;
        try {
          result = await target.complete(providerRequest);
        } catch (error) {
          await controls.lifecycle.failed();
          throw error;
        }
        const current = usage(result.usage, options.cost);
        metered = addUsage(metered, current);
        await options.onUsage?.(current);
        await controls.lifecycle.succeeded(result.finishReason);
        return result;
      };
    },
  });
  const agent = createAgent({
    provider,
    model: options.model,
    system: request.system,
    effort: 'medium',
    messages: createMessageStorage(),
    tools: createToolStorage([]),
    flags: { sensitiveOutput: true },
  });
  const result = await agent.complete(request.user, {
    schema,
    maxTurns: controls.maxTurns,
    maxToolCallRepairs: controls.structuredRepairRetries,
    onStructuredAttempt: ({
      attempt,
      runtimeAccepted,
      feedbackSent,
      diagnostic,
    }) =>
      controls.lifecycle.structured({
        attempt,
        runtimeAccepted,
        feedbackSent,
        ...(diagnostic === undefined ? {} : { diagnostic }),
      }),
    onToolCallRepair: ({ attempt, maxAttempts }) =>
      controls.lifecycle.repair({ attempt, maxAttempts }),
  });
  return { value: schema.parse(result.structured), usage: metered };
};

/** Adapts the provider-neutral structured API to the frozen baseline model contract. */
export const createProviderBaselineModel = (
  options: ProviderBaselineOptions,
): BaselineModel => ({
  observesProviderCalls: true,
  plan: (request): Promise<ModelAnswer<BaselinePlan>> =>
    complete(options, plan, request),
  execute: (request): Promise<ModelAnswer<BaselineDecision>> =>
    complete(options, decision, request),
  revise: (request): Promise<ModelAnswer<PlannedGoal>> =>
    complete(options, goal, request),
});
