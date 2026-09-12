import { createStateMachine } from 'state-machine';

import { bundle } from '../states/bundle/index.js';
import { delivery } from '../states/delivery/index.js';
import { execution } from '../states/execution/index.js';
import { plan } from '../states/plan/index.js';
import { revision } from '../states/revision/index.js';
import { schedule } from '../states/schedule/index.js';
import type { MosaicResult } from '../types/result.js';
import type {
  WorkflowContext,
  WorkflowHandler,
  WorkflowHandlerName,
  WorkflowState,
} from '../types/workflow.js';

/**
 * Creates the reusable MOSAIC lifecycle: plan(P0) -> plan(P1) -> schedule ->
 * bundle -> execution -> schedule -> delivery, with schedule -> revision -> schedule when
 * node evidence invalidates the active graph.
 */
export const createMachine = () =>
  createStateMachine<WorkflowContext, WorkflowState, MosaicResult>()({
    plan: observed('plan', plan),
    schedule: observed('schedule', schedule),
    revision: observed('revision', revision),
    bundle: observed('bundle', bundle),
    execution: observed('execution', execution),
    delivery: observed('delivery', delivery),
  });

const observed =
  (stage: WorkflowHandlerName, handler: WorkflowHandler): WorkflowHandler =>
  async (state, context, actions) => {
    const runtime = context.runtime;

    if (runtime === undefined) {return handler(state, context, actions);}

    const timer = runtime.timer();

    await runtime.emit({ type: 'stage.started', stage });

    const action = await handler(state, context, actions);

    await runtime.emit({
      type: action.type === 'fail' ? 'stage.failed' : 'stage.finished',
      stage,
      durationMs: runtime.duration(timer),
    });

    return action;
  };
