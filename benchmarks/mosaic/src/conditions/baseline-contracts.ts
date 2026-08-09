import type { ToolName } from '../config/index.js';
import type { JsonValue } from '../core/json.js';
import type { EngineUsage } from '../runtime/index.js';

export interface PlannedGoal {
  readonly id: string;
  readonly goal: string;
  readonly doneWhen: readonly string[];
}

export interface BaselinePlan {
  readonly goals: readonly PlannedGoal[];
}

export interface ToolRequest {
  readonly name: string;
  readonly input: JsonValue;
}

export interface BaselineDecision {
  readonly status: 'completed' | 'failed' | 'needs_revision';
  readonly output: string;
  readonly toolCalls: readonly ToolRequest[];
  readonly revisionReason?: string;
}

export interface ModelAnswer<Value> {
  readonly value: Value;
  readonly usage: EngineUsage;
}

export interface BaselineCallLifecycle {
  readonly started: (details?: {
    readonly messageCount: number;
    readonly toolNames: readonly string[];
  }) => Promise<void>;
  readonly succeeded: (finishReason: string) => Promise<void>;
  readonly failed: () => Promise<void>;
  readonly structured: (event: {
    readonly attempt: number;
    readonly runtimeAccepted: boolean;
    readonly feedbackSent: boolean;
    readonly diagnostic?: string;
  }) => Promise<void>;
  readonly repair: (event: {
    readonly attempt: number;
    readonly maxAttempts: number;
  }) => Promise<void>;
}

export interface BaselineRunControls {
  readonly maxTurns: number;
  readonly structuredRepairRetries: number;
  readonly lifecycle: BaselineCallLifecycle;
}

interface ModelRequest {
  readonly system: string;
  readonly user: string;
  readonly controls?: BaselineRunControls;
}

export interface BaselineModel {
  readonly observesProviderCalls?: boolean;
  readonly plan: (request: ModelRequest) => Promise<ModelAnswer<BaselinePlan>>;
  readonly execute: (
    request: ModelRequest & { readonly toolNames: readonly ToolName[] },
  ) => Promise<ModelAnswer<BaselineDecision>>;
  readonly revise: (request: ModelRequest) => Promise<ModelAnswer<PlannedGoal>>;
}
