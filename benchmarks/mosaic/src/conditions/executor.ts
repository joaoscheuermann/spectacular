import type { EngineResult, RunContext } from '../runtime/index.js';
import { executeBaseline, type BaselineModel } from './baselines.js';
import { executeMosaic, type MosaicDependencies } from './mosaic.js';
import type { ConditionPrompts } from './prompts.js';
import { createSkillRetrieval, type SkillRetrieval } from './retrieval.js';

export interface ConditionExecutorDependencies {
  readonly baseline: BaselineModel;
  readonly prompts: ConditionPrompts;
  readonly mosaic: MosaicDependencies;
  readonly retrieval?: SkillRetrieval;
}

const defaultRetrieval = (dependencies: MosaicDependencies): SkillRetrieval =>
  createSkillRetrieval({
    models: {
      embedder: dependencies.base.models.embedder,
      reranker: dependencies.base.models.reranker,
    },
    search: dependencies.retrievers.skills.search.bind(
      dependencies.retrievers.skills,
    ),
    rerank: dependencies.base.provider.rerank.bind(dependencies.base.provider),
  });

/** Selects the frozen implementation for the run's condition ID. */
export const createConditionExecutor =
  (
    dependencies: ConditionExecutorDependencies,
  ): ((context: RunContext) => Promise<EngineResult>) =>
  (context) => {
    const retrieval =
      dependencies.retrieval ?? defaultRetrieval(dependencies.mosaic);
    return ['B0', 'B1', 'B2', 'B3'].includes(context.condition.id)
      ? executeBaseline(
          context,
          dependencies.baseline,
          dependencies.prompts,
          retrieval,
        )
      : executeMosaic(context, dependencies.mosaic, retrieval);
  };
