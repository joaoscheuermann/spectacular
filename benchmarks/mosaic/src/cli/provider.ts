import type {
  LlmProvider,
  ProviderFinished,
  ProviderRequest,
  ProviderStreamEvent,
  UsageMetadata,
} from 'llms';

import type { EngineUsage } from '../runtime/index.js';
import { operationCost, type BillableUsage, type Prices } from './pricing.js';
import type { RetrievalUsage } from './usage-transport.js';

export type MeterOperation = 'completion' | 'embedding' | 'rerank';
export type MeterStage = 'index' | 'probe' | 'run';

export interface MeterUsage extends EngineUsage {
  readonly embeddingInputTokens: number;
  readonly rerankInputTokens: number;
  readonly rerankDocuments: number;
  readonly rerankSearchUnits: number;
  readonly completionRequests: number;
  readonly embeddingRequests: number;
  readonly rerankRequests: number;
}

export interface MeterSink {
  readonly append: (entry: {
    readonly operation: MeterOperation;
    readonly stage: MeterStage;
    readonly model: string;
    readonly usage: MeterUsage;
    readonly costUsd: number;
  }) => Promise<unknown>;
}

export interface MeteredProvider {
  readonly provider: LlmProvider;
  readonly reset: (usage?: MeterUsage) => void;
  readonly bind: (sink: MeterSink | undefined) => void;
  readonly stage: (stage: MeterStage) => void;
  readonly snapshot: () => MeterUsage;
  readonly costFor: (model: string, usage: CompletionUsage) => number;
  readonly recordRetrieval: (usage: RetrievalUsage) => Promise<void>;
}

export interface CompletionUsage {
  readonly inputTokens: number;
  readonly outputTokens: number;
  readonly cachedInputTokens?: number;
  readonly reasoningTokens?: number;
}

export interface MeterOptions {
  readonly retrievalUsage?: (
    operation: RetrievalUsage['operation'],
    model: string,
  ) => RetrievalUsage | undefined;
  readonly retrievalCaptured?: boolean;
}

const zero = (): MeterUsage => ({
  inputTokens: 0,
  outputTokens: 0,
  cachedInputTokens: 0,
  reasoningTokens: 0,
  embeddingInputTokens: 0,
  rerankInputTokens: 0,
  rerankDocuments: 0,
  rerankSearchUnits: 0,
  completionRequests: 0,
  embeddingRequests: 0,
  rerankRequests: 0,
  costUsd: 0,
});

const usageCount = (value: number | undefined, field: string): number => {
  if (value === undefined) return 0;
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new TypeError(`provider returned invalid ${field} usage`);
  }
  return value;
};

const completionUsage = (usage: UsageMetadata | undefined): CompletionUsage => {
  if (usage?.inputTokens === undefined || usage.outputTokens === undefined) {
    throw new TypeError('provider omitted authenticated completion usage');
  }
  return {
    inputTokens: usageCount(usage.inputTokens, 'input token'),
    outputTokens: usageCount(usage.outputTokens, 'output token'),
    cachedInputTokens: usageCount(
      usage.cachedInputTokens,
      'cached input token',
    ),
    reasoningTokens: usageCount(usage.reasoningTokens, 'reasoning token'),
  };
};

const mediumRequest = <Output>(
  request: ProviderRequest<Output>,
): ProviderRequest<Output> => ({
  ...request,
  effort: 'medium',
  flags: { ...request.flags, includeUsage: true },
});

const deltaFor = (
  operation: MeterOperation,
  billable: BillableUsage,
  costUsd: number,
): MeterUsage => ({
  ...zero(),
  inputTokens: billable.inputTokens ?? 0,
  outputTokens: billable.outputTokens ?? 0,
  cachedInputTokens: billable.cachedInputTokens ?? 0,
  reasoningTokens: 0,
  embeddingInputTokens: billable.embeddingInputTokens ?? 0,
  rerankInputTokens: billable.rerankInputTokens ?? 0,
  rerankDocuments: billable.documents ?? 0,
  rerankSearchUnits: billable.rerankSearchUnits ?? 0,
  completionRequests: operation === 'completion' ? 1 : 0,
  embeddingRequests: operation === 'embedding' ? 1 : 0,
  rerankRequests: operation === 'rerank' ? 1 : 0,
  costUsd,
});

const sum = (left: MeterUsage, right: MeterUsage): MeterUsage => ({
  inputTokens: left.inputTokens + right.inputTokens,
  outputTokens: left.outputTokens + right.outputTokens,
  cachedInputTokens:
    (left.cachedInputTokens ?? 0) + (right.cachedInputTokens ?? 0),
  reasoningTokens: (left.reasoningTokens ?? 0) + (right.reasoningTokens ?? 0),
  embeddingInputTokens: left.embeddingInputTokens + right.embeddingInputTokens,
  rerankInputTokens: left.rerankInputTokens + right.rerankInputTokens,
  rerankDocuments: left.rerankDocuments + right.rerankDocuments,
  rerankSearchUnits: left.rerankSearchUnits + right.rerankSearchUnits,
  completionRequests: left.completionRequests + right.completionRequests,
  embeddingRequests: left.embeddingRequests + right.embeddingRequests,
  rerankRequests: left.rerankRequests + right.rerankRequests,
  costUsd: left.costUsd + right.costUsd,
});

/** Forces frozen effort and meters authenticated usage by operation. */
export const createMeteredProvider = (
  source: LlmProvider,
  prices: Prices,
  options: MeterOptions = {},
): MeteredProvider => {
  let current = zero();
  let sink: MeterSink | undefined;
  let stage: MeterStage = 'run';
  const add = async (
    operation: MeterOperation,
    model: string,
    billable: BillableUsage,
    reasoningTokens = 0,
  ): Promise<void> => {
    const costUsd = operationCost(prices, model, billable);
    const delta = {
      ...deltaFor(operation, billable, costUsd),
      reasoningTokens,
    };
    await sink?.append({ operation, stage, model, usage: delta, costUsd });
    current = sum(current, delta);
  };
  const addCompletion = async (
    model: string,
    metadata: UsageMetadata | undefined,
  ): Promise<void> => {
    const usage = completionUsage(metadata);
    await add(
      'completion',
      model,
      { ...usage, requests: 1 },
      usage.reasoningTokens ?? 0,
    );
  };
  const takeRetrieval = async (
    operation: RetrievalUsage['operation'],
    model: string,
    required: boolean,
  ): Promise<void> => {
    const usage = options.retrievalUsage?.(operation, model);
    if (usage === undefined) {
      if (required) {
        throw new TypeError(
          `provider omitted authenticated ${operation} usage`,
        );
      }
      return;
    }
    if (options.retrievalCaptured !== true) await recordRetrieval(usage);
  };
  const recordRetrieval = async (usage: RetrievalUsage): Promise<void> =>
    add(
      usage.operation,
      usage.model,
      usage.operation === 'embedding'
        ? { embeddingInputTokens: usage.inputTokens, requests: 1 }
        : {
            rerankInputTokens: usage.inputTokens,
            rerankSearchUnits: usage.searchUnits,
            documents: usage.documents,
            requests: 1,
          },
    );
  const complete = (async (request: ProviderRequest) => {
    const result = await source.complete(mediumRequest(request) as never);
    await addCompletion(request.model, result.usage);
    return result;
  }) as LlmProvider['complete'];
  const stream = ((request: ProviderRequest) => {
    const events = source.stream(mediumRequest(request) as never);
    return (async function* (): AsyncIterable<ProviderStreamEvent<unknown>> {
      for await (const event of events) {
        if (event.type === 'response.finished') {
          await addCompletion(request.model, event.finish.usage);
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
      try {
        const result = await source.embedding(request);
        await takeRetrieval('embedding', request.model, true);
        return result;
      } catch (error) {
        await takeRetrieval('embedding', request.model, false);
        throw error;
      }
    },
    async rerank(request) {
      try {
        const result = await source.rerank(request);
        await takeRetrieval('rerank', request.model, true);
        return result;
      } catch (error) {
        await takeRetrieval('rerank', request.model, false);
        throw error;
      }
    },
    models: (signal) => source.models(signal),
    validateModel: (model, signal) => source.validateModel(model, signal),
  };
  return {
    provider,
    reset: (usage = zero()) => {
      current = { ...usage };
    },
    bind: (value) => {
      sink = value;
    },
    stage: (value) => {
      stage = value;
    },
    snapshot: () => ({ ...current }),
    costFor: (model, usage) =>
      operationCost(prices, model, { ...usage, requests: 1 }),
    recordRetrieval,
  };
};

export const finishedUsage = (result: ProviderFinished): CompletionUsage =>
  completionUsage(result.usage);
