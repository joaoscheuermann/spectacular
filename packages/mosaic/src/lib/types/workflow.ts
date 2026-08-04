import type { StateMachineHandler } from 'state-machine';

import type { Graph, Node } from './graph.js';
import type { MosaicOptions } from './mosaic-options.js';

export type WorkflowState = 'decompose' | 'schedule' | 'prepare';

export type WorkflowContext = {
  readonly input: string;
  readonly options: MosaicOptions;
};

export type WorkflowArtifacts = {
  readonly decompose: undefined;
  readonly schedule: {
    readonly graph: Graph;
  };
  readonly prepare: {
    readonly graph: Graph;
    readonly nodes: readonly Node[];
  };
};

export type WorkflowHandler<State extends WorkflowState> = StateMachineHandler<
  WorkflowState,
  State,
  WorkflowContext,
  WorkflowArtifacts,
  void,
  unknown
>;
