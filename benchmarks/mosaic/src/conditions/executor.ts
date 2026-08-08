import type { EngineResult, RunContext } from '../runtime/index.js';
import { executeBaseline, type BaselineModel } from './baselines.js';
import { executeMosaic, type MosaicDependencies } from './mosaic.js';
import type { ConditionPrompts } from './prompts.js';

export interface ConditionExecutorDependencies {
  readonly baseline: BaselineModel;
  readonly prompts: ConditionPrompts;
  readonly mosaic: MosaicDependencies;
}

/** Selects the frozen implementation for the run's condition ID. */
export const createConditionExecutor =
  (
    dependencies: ConditionExecutorDependencies,
  ): ((context: RunContext) => Promise<EngineResult>) =>
  (context) =>
    ['B0', 'B1', 'B2', 'B3'].includes(context.condition.id)
      ? executeBaseline(context, dependencies.baseline, dependencies.prompts)
      : executeMosaic(context, dependencies.mosaic);
