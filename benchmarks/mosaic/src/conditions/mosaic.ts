import type { MosaicEvent, MosaicOptions, MosaicResult } from 'mosaic';
import { mosaic as createMosaic } from 'mosaic/evaluation';

import type { JsonValue } from '../core/json.js';
import {
  HarnessInfrastructureError,
  ModelCallBudgetError,
  type EngineResult,
  type RunContext,
} from '../runtime/index.js';
import {
  createToolCorrelations,
  mosaicOptions,
  type MosaicDependencies,
} from './mosaic-adapters.js';
import { evaluationHooks } from './mosaic-hooks.js';
import { isProviderError } from './provider.js';
import type { SkillRetrieval } from './retrieval.js';

export * from './mosaic-adapters.js';
export * from './mosaic-hooks.js';

const eventJson = (
  event: MosaicEvent,
  routing: MosaicOptions['routing'],
): JsonValue => {
  const value = JSON.parse(JSON.stringify(event)) as JsonValue;
  if (
    event.type !== 'retrieval.result' ||
    typeof value !== 'object' ||
    value === null ||
    Array.isArray(value)
  ) {
    return value;
  }
  return {
    ...value,
    k:
      event.stage === 'plan'
        ? routing.maxHintCandidates
        : routing.maxRetrievedCandidates,
  };
};

const resultJson = (result: MosaicResult): JsonValue =>
  JSON.parse(JSON.stringify(result)) as JsonValue;

const observedRetriever = <Value>(
  context: RunContext,
  retriever: {
    search(
      query: string,
      topK: number,
    ): Promise<ReadonlyArray<{ readonly data: Value; readonly score: number }>>;
  },
  model: string | undefined,
) =>
  model === undefined
    ? retriever
    : {
        search: async (query: string, topK: number) => {
          await context.startModelCall();
          await context.emit({
            type: 'model.request',
            stage: 'retrieval',
            operation: 'embedding',
            model,
            messageCount: 1,
            query,
          });
          const started = Date.now();
          try {
            const matches = await retriever.search(query, topK);
            await context.emit({
              type: 'model.response',
              stage: 'retrieval',
              operation: 'embedding',
              model,
              resultCount: matches.length,
              durationMs: Math.max(0, Date.now() - started),
            });
            return matches;
          } catch (error) {
            await context.emit({
              type: 'model.failed',
              stage: 'retrieval',
              operation: 'embedding',
              model,
              status: 'failed',
              durationMs: Math.max(0, Date.now() - started),
            });
            throw error;
          }
        },
      };

const observedDependencies = (
  context: RunContext,
  dependencies: MosaicDependencies,
): MosaicDependencies => ({
  ...dependencies,
  retrievers: {
    skills: observedRetriever(
      context,
      dependencies.retrievers.skills,
      dependencies.retrieverModels?.skills,
    ),
    metadataSkills: observedRetriever(
      context,
      dependencies.retrievers.metadataSkills,
      dependencies.retrieverModels?.metadataSkills,
    ),
    tools: observedRetriever(
      context,
      dependencies.retrievers.tools,
      dependencies.retrieverModels?.tools,
    ),
  },
});

const observedSkillRetrieval = (
  context: RunContext,
  retrieval: SkillRetrieval,
  model: string,
): SkillRetrieval => ({
  models: retrieval.models,
  search: async (query, topK) => {
    const observed = observedRetriever(
      context,
      {
        search: async (value: string, limit: number) =>
          (await retrieval.search(value, limit)).map((data) => ({
            data,
            score: data.score,
          })),
      },
      model,
    );
    return (await observed.search(query, topK)).map(({ data }) => data);
  },
  rerank: retrieval.rerank,
});

/** Executes M0/M1 and diagnostic variants through the public evaluation entrypoint. */
export const executeMosaic = async (
  context: RunContext,
  dependencies: MosaicDependencies,
  retrieval?: SkillRetrieval,
): Promise<EngineResult> => {
  const observed = observedDependencies(context, dependencies);
  const factory = observed.factory ?? createMosaic;
  const correlations = createToolCorrelations();
  const shared =
    retrieval === undefined
      ? undefined
      : observedSkillRetrieval(
          context,
          retrieval,
          observed.retrieverModels?.skills ?? observed.base.models.embedder,
        );
  const options = mosaicOptions(context, observed, correlations, shared);
  const hooks = evaluationHooks(
    context.condition,
    context.benchmarkCase,
    context.run.seed,
    observed.oracles,
    observed,
    context.startModelCall,
    context.emit,
    context,
  );
  const agent = factory(options, { hooks });
  let result: MosaicResult;
  try {
    result = await agent.prompt(context.benchmarkCase.request, {
      capture: context.run.capture,
      observer: async (event) => {
        try {
          if (event.type === 'model.request') await context.startModelCall();
          if (event.type === 'tool.started') correlations.started(event);
          await context.emit(eventJson(event, options.routing));
        } catch (error) {
          if (
            error instanceof HarnessInfrastructureError ||
            error instanceof ModelCallBudgetError
          ) {
            throw error;
          }
          throw new HarnessInfrastructureError('observer', 'observer_failed');
        }
      },
    });
  } catch (error) {
    if (
      error instanceof HarnessInfrastructureError ||
      error instanceof ModelCallBudgetError
    ) {
      throw error;
    }
    if (isProviderError(error)) {
      throw new HarnessInfrastructureError('model', 'provider_failed');
    }
    throw error;
  }
  return {
    status: result.status === 'completed' ? 'succeeded' : 'failed',
    outcome: resultJson(result),
    usage: observed.usage?.() ?? {
      inputTokens: 0,
      outputTokens: 0,
      costUsd: 0,
    },
  };
};
