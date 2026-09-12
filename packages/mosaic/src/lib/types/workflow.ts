import type { StateMachineHandler } from 'state-machine';

import type { MosaicRuntime } from '../observability.js';
import type { ObservationIdAllocator } from '../observation-ids.js';
import type { MosaicEvaluationHooks } from './evaluation.js';
import type { Graph } from './graph.js';
import type { MosaicOptions } from './mosaic-options.js';
import type { MosaicResult } from './result.js';

export type WorkflowHandlerName =
  | 'plan'
  | 'schedule'
  | 'revision'
  | 'bundle'
  | 'execution'
  | 'delivery';

export type WorkflowContext = {
  readonly input: string;
  readonly options: MosaicOptions;
  readonly runtime?: MosaicRuntime;
  readonly hooks?: MosaicEvaluationHooks;
  readonly observationIds?: ObservationIdAllocator;
};

export type WorkflowState = {
  graphs: Graph[];
};

export type WorkflowHandler = StateMachineHandler<
  WorkflowContext,
  WorkflowState,
  WorkflowHandlerName,
  MosaicResult
>;
