import type { ToolName } from '../config/index.js';
import {
  HarnessInfrastructureError,
  ModelCallBudgetError,
  type RunContext,
} from '../runtime/index.js';
import type {
  BaselineCallLifecycle,
  BaselineModel,
  BaselineRunControls,
  ModelAnswer,
} from './baseline-contracts.js';
import { isAgentErrorCode, isProviderError } from './provider.js';

interface ModelRequest {
  readonly system: string;
  readonly user: string;
  readonly toolNames?: readonly ToolName[];
}

/** Observes and counts every provider invocation in one baseline operation. */
export const callModel = async <Value>(
  context: RunContext,
  model: BaselineModel,
  stage: 'plan' | 'execution' | 'revision',
  operation: 'plan' | 'execute' | 'revise',
  request: ModelRequest,
  maxTurns: number,
  invoke: (controls: BaselineRunControls) => Promise<ModelAnswer<Value>>,
): Promise<{ readonly answer: ModelAnswer<Value>; readonly calls: number }> => {
  let calls = 0;
  let failed = false;
  const lifecycle: BaselineCallLifecycle = {
    started: async (details) => {
      await context.startModelCall();
      calls += 1;
      await context.emit({
        type: 'model.request',
        stage,
        operation,
        model: context.run.model.model,
        messageCount: details?.messageCount ?? 2,
        toolNames: details?.toolNames ?? request.toolNames ?? [],
        prompt: { system: request.system, user: request.user },
      });
    },
    succeeded: (finishReason) =>
      context.emit({
        type: 'model.response',
        stage,
        operation,
        model: context.run.model.model,
        finishReason,
        durationMs: 0,
      }),
    failed: async () => {
      failed = true;
      await context.emit({
        type: 'model.failed',
        stage,
        operation,
        model: context.run.model.model,
        status: 'failed',
        durationMs: 0,
      });
    },
    structured: (event) =>
      context.emit({ type: 'structured.attempt', stage, ...event }),
    repair: (event) => context.emit({ type: 'tool.repair', stage, ...event }),
  };
  const controls = {
    maxTurns,
    structuredRepairRetries: context.condition.factors.structuredRepairRetries,
    lifecycle,
  };
  try {
    if (model.observesProviderCalls !== true) await lifecycle.started();
    const answer = await invoke(controls);
    if (model.observesProviderCalls !== true) {
      await lifecycle.structured({
        attempt: 1,
        runtimeAccepted: true,
        feedbackSent: false,
      });
      await lifecycle.succeeded('structured');
    }
    return { answer, calls };
  } catch (error) {
    if (!failed) await lifecycle.failed();
    if (
      error instanceof HarnessInfrastructureError ||
      error instanceof ModelCallBudgetError
    ) {
      throw error;
    }
    if (
      isProviderError(error) ||
      isAgentErrorCode(error, 'invalid_structured_output')
    ) {
      throw new HarnessInfrastructureError('model', 'provider_failed');
    }
    throw error;
  }
};
