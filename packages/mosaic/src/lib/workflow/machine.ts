import { createStateMachine } from 'state-machine';

import { graph } from '../states/graph/index.js';
import { bundle } from '../states/bundle/index.js';
import { schedule } from '../states/schedule/index.js';
import type { WorkflowContext, WorkflowState } from '../types/workflow.js';

/** Creates the reusable Mosaic planning and preparation state machine. */
export const createMachine = () =>
  createStateMachine<WorkflowContext, WorkflowState>()({
    graph,
    schedule,
    bundle,
  });
