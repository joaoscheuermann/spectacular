import type { Graph } from './graph.js';
import type { MosaicOptions } from './mosaic-options.js';
import type { MosaicResult } from './result.js';
import type { StateMachineHandler } from 'state-machine';

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
