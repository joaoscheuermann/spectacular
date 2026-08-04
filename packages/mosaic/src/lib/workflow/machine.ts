import { createStateMachine } from 'state-machine';

import { decompose } from '../states/decompose/index.js';
import { prepare } from '../states/prepare/index.js';
import { schedule } from '../states/schedule/index.js';
import type {
  WorkflowArtifacts,
  WorkflowContext,
  WorkflowState,
} from '../types/workflow.js';

/** Creates the reusable Mosaic planning and preparation state machine. */
export const createMachine = () =>
  createStateMachine<
    WorkflowState,
    WorkflowContext,
    WorkflowArtifacts,
    void,
    unknown
  >({
    decompose,
    schedule,
    prepare,
  });
