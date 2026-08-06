import type { StateMachineHandlers } from 'state-machine';

import type { Graph } from './graph.js';
import type { MosaicOptions } from './mosaic-options.js';

export type WorkflowHandlerName = 'graph' | 'schedule' | 'prepare';

export type WorkflowContext = {
  readonly input: string;
  readonly options: MosaicOptions;
};

export type WorkflowState = {
  graphs: Graph[];
};

export type WorkflowHandler<Handler extends WorkflowHandlerName> =
  StateMachineHandlers<
    WorkflowHandlerName,
    WorkflowState,
    WorkflowContext,
    void,
    unknown
  >[Handler];
