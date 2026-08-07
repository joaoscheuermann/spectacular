import type { Graph } from './graph.js';
import type { MosaicOptions } from './mosaic-options.js';

export type WorkflowHandlerName =
  | 'plan'
  | 'schedule'
  | 'revision'
  | 'bundle'
  | 'execution';

export type WorkflowContext = {
  readonly input: string;
  readonly options: MosaicOptions;
};

export type WorkflowState = {
  graphs: Graph[];
};
