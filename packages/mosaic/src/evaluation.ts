import { createMosaic } from './lib/mosaic.js';
import type {
  EvaluationHook,
  ExecutionInput,
  ExecutionResult,
  FeedbackPlanInput,
  FeedbackPlanResult,
  InitialPlanInput,
  LocalizedRevisionInput,
  MenuInput,
  MosaicEvaluationHooks,
  MosaicEvaluationOptions,
  RetrievalInput,
  RoutingInput,
  SkillMatch,
  SkillViewInput,
} from './lib/types/evaluation.js';
import type { MosaicAgent } from './lib/types/mosaic-agent.js';
import type { MosaicOptions } from './lib/types/mosaic-options.js';

/** Creates MOSAIC with validated experimental interception hooks. */
export const mosaic = (
  options: MosaicOptions,
  evaluation: MosaicEvaluationOptions = {},
): MosaicAgent => createMosaic(options, evaluation.hooks);

export type {
  EvaluationHook,
  ExecutionInput,
  ExecutionResult,
  FeedbackPlanInput,
  FeedbackPlanResult,
  InitialPlanInput,
  LocalizedRevisionInput,
  MenuInput,
  MosaicEvaluationHooks,
  MosaicEvaluationOptions,
  RetrievalInput,
  RoutingInput,
  SkillMatch,
  SkillViewInput,
};
