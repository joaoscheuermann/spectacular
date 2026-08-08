import { createStateMachine } from 'state-machine';

import { plan } from '../states/plan/index.js';
import { bundle } from '../states/bundle/index.js';
import { execution } from '../states/execution/index.js';
import { delivery } from '../states/delivery/index.js';
import { revision } from '../states/revision/index.js';
import { schedule } from '../states/schedule/index.js';
import type { WorkflowContext, WorkflowState } from '../types/workflow.js';
import type { MosaicResult } from '../types/result.js';

/**
 * Creates the reusable MOSAIC lifecycle: plan(P0) -> plan(P1) -> schedule ->
 * bundle -> execution -> schedule -> delivery, with schedule -> revision -> schedule when
 * node evidence invalidates the active graph.
 */
export const createMachine = () =>
  createStateMachine<WorkflowContext, WorkflowState, MosaicResult>()({
    plan,
    schedule,
    revision,
    bundle,
    execution,
    delivery,
  });
