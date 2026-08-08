import {
  ExecutionRecordV1,
  type Case,
  type Condition,
  type ExecutionRecord,
  type RunSpec,
} from '../schemas/index.js';
import type { JsonValue } from '../core/json.js';
import { artifactHash } from '../core/hash.js';
import type { ToolName } from '../config/index.js';
import { boundaryPolicy, oracleEligible } from '../conditions/index.js';
import { executeTool } from './tools.js';
import { createWorld, type World, worldHash } from './world.js';
import type { EventStore, RecordStore } from './store.js';

export type CrashBoundary =
  | 'before-prepare'
  | 'before-first-model'
  | 'after-first-model'
  | 'after-tool'
  | 'before-terminal'
  | 'after-terminal';

export type InfrastructureStage =
  | 'prepare'
  | 'model'
  | 'tool'
  | 'observer'
  | 'store'
  | 'score'
  | 'unknown';

const INFRASTRUCTURE_CODES = new Set([
  'event_append_failed',
  'injected_crash',
  'interrupted',
  'observer_failed',
  'prepare_failed',
  'provider_failed',
  'record_append_failed',
  'record_read_failed',
  'score_failed',
  'tool_failed',
  'trace_derive_failed',
  'unclassified',
]);

export class HarnessInfrastructureError extends Error {
  readonly stage: InfrastructureStage;
  readonly code: string;

  constructor(stage: InfrastructureStage, code: string) {
    super('benchmark infrastructure failure');
    this.name = 'HarnessInfrastructureError';
    this.stage = stage;
    this.code = INFRASTRUCTURE_CODES.has(code) ? code : 'unclassified';
  }
}

export class ModelCallBudgetError extends Error {
  constructor() {
    super('model call budget exhausted');
    this.name = 'ModelCallBudgetError';
  }
}

export interface EngineUsage {
  readonly inputTokens: number;
  readonly outputTokens: number;
  readonly cachedInputTokens?: number;
  readonly reasoningTokens?: number;
  readonly costUsd: number;
}

export interface EngineResult {
  readonly status: 'succeeded' | 'failed';
  readonly outcome: JsonValue | null;
  readonly usage: EngineUsage;
}

export interface ToolCallCorrelation {
  readonly callId?: string;
  readonly nodeId?: string;
  readonly revision?: number;
}

export interface RunContext {
  readonly run: RunSpec;
  readonly benchmarkCase: Case;
  readonly condition: Condition;
  readonly policy: ReturnType<typeof boundaryPolicy>;
  readonly emit: (payload: JsonValue) => Promise<void>;
  readonly startModelCall: () => Promise<void>;
  readonly callTool: (
    name: ToolName,
    input: JsonValue,
    correlation?: ToolCallCorrelation,
  ) => Promise<JsonValue>;
  readonly world: () => World;
}

export interface RunnerDependencies {
  readonly events: EventStore;
  readonly records: RecordStore;
  readonly execute: (context: RunContext) => Promise<EngineResult>;
  /** Returns cumulative, allowlisted metering even when execution throws. */
  readonly usage?: () => EngineUsage;
  readonly now?: () => Date;
  readonly crash?: (boundary: CrashBoundary) => void | Promise<void>;
}

const failureDetails = (
  error: unknown,
): { readonly stage: InfrastructureStage; readonly code: string } =>
  error instanceof HarnessInfrastructureError
    ? { stage: error.stage, code: error.code }
    : { stage: 'unknown', code: 'unclassified' };

const iso = (date: Date): string => date.toISOString();

const STRUCTURE_KEYS = new Set([
  'attempt',
  'callId',
  'candidateNames',
  'caseId',
  'conditionId',
  'count',
  'diagnostic',
  'durationMs',
  'eventCount',
  'evidenceHash',
  'feedbackSent',
  'finishReason',
  'goalId',
  'hintCount',
  'k',
  'infrastructure',
  'messageCount',
  'model',
  'modelCall',
  'name',
  'nodeId',
  'nodeIds',
  'operation',
  'partIds',
  'phase',
  'rank',
  'ranking',
  'revision',
  'resultCount',
  'runId',
  'runtimeAccepted',
  'schemaVersion',
  'score',
  'skillName',
  'skillNames',
  'stage',
  'status',
  'toolName',
  'toolNames',
  'type',
]);
const IO_KEYS = new Set([
  'bundle',
  'candidates',
  'content',
  'decision',
  'delivery',
  'graph',
  'hints',
  'input',
  'observations',
  'outcome',
  'output',
  'payload',
  'prompt',
  'query',
  'response',
]);

const sensitiveKey = (key: string): boolean => {
  const value = key.replace(/[^a-z0-9]/giu, '').toLocaleLowerCase('en-US');
  return (
    /(?:authorization|credentials?|password|secret|privatekey|apikey|accesstoken|refreshtoken|idtoken|reasoning|replay|encryptedcontent|cause|stack|error|headers|cookie)/u.test(
      value,
    ) || ['auth', 'bearer', 'key', 'sessiontoken', 'token'].includes(value)
  );
};

const sanitizeIo = (value: JsonValue): JsonValue => {
  if (Array.isArray(value)) return value.map(sanitizeIo);
  if (typeof value !== 'object' || value === null) return value;
  return Object.fromEntries(
    Object.entries(value)
      .filter(([key]) => !sensitiveKey(key))
      .map(([key, entry]) => [key, sanitizeIo(entry)]),
  );
};

const structureValue = (value: JsonValue): JsonValue => {
  if (Array.isArray(value)) return value.map(structureValue);
  if (typeof value !== 'object' || value === null) return value;
  return Object.fromEntries(
    Object.entries(value)
      .filter(([key]) => STRUCTURE_KEYS.has(key) && !sensitiveKey(key))
      .map(([key, entry]) => [key, structureValue(entry)]),
  );
};

const projectCapture = (
  value: JsonValue,
  capture: 'structure' | 'io',
): JsonValue => {
  if (Array.isArray(value))
    return value.map((entry) => projectCapture(entry, capture));
  if (typeof value !== 'object' || value === null) return value;
  return Object.fromEntries(
    Object.entries(value).flatMap(([key, entry]) => {
      if (sensitiveKey(key)) return [];
      if (STRUCTURE_KEYS.has(key)) return [[key, structureValue(entry)]];
      if (capture === 'io' && IO_KEYS.has(key))
        return [[key, sanitizeIo(entry)]];
      return [];
    }),
  );
};

/** Executes one isolated run and persists its terminal infrastructure outcome. */
export const executeRun = async (
  run: RunSpec,
  benchmarkCase: Case,
  condition: Condition,
  dependencies: RunnerDependencies,
  oracleParent?: ExecutionRecord,
): Promise<ExecutionRecord> => {
  if (!oracleEligible(condition, benchmarkCase, oracleParent)) {
    throw new TypeError('failure-only oracle requires a failed parent record');
  }
  const clock = dependencies.now ?? (() => new Date());
  const priorAttempts = await dependencies.records.read(run.id).catch(() => {
    throw new HarnessInfrastructureError('store', 'record_read_failed');
  });
  const attempt = priorAttempts.length + 1;
  const started = clock();
  let world = createWorld();
  let firstModelCallStarted = false;
  let modelCalls = 0;
  let toolCalls = 0;
  const emit = async (payload: JsonValue): Promise<void> => {
    try {
      await dependencies.events.append(
        run.id,
        attempt,
        projectCapture(payload, run.capture),
      );
    } catch {
      throw new HarnessInfrastructureError('store', 'event_append_failed');
    }
  };
  const crash = async (boundary: CrashBoundary): Promise<void> => {
    await dependencies.crash?.(boundary);
  };
  const startModelCall = async (): Promise<void> => {
    if (
      run.modelCallBudget !== undefined &&
      modelCalls >= run.modelCallBudget
    ) {
      throw new ModelCallBudgetError();
    }
    await crash('before-first-model');
    firstModelCallStarted = true;
    modelCalls += 1;
    await emit({ type: 'model.call.started', modelCall: modelCalls });
    if (modelCalls === 1) await crash('after-first-model');
  };
  const callTool = async (
    name: ToolName,
    input: JsonValue,
    correlation: ToolCallCorrelation = {},
  ): Promise<JsonValue> => {
    toolCalls += 1;
    const identity = {
      callId:
        correlation.callId ?? `tool-call-${String(toolCalls).padStart(6, '0')}`,
      ...(correlation.nodeId === undefined
        ? {}
        : { nodeId: correlation.nodeId }),
      ...(correlation.revision === undefined
        ? {}
        : { revision: correlation.revision }),
    };
    await emit({ type: 'tool.call.started', name, ...identity, input });
    let execution;
    try {
      execution = executeTool(world, name, input);
    } catch {
      await emit({
        type: 'tool.call.failed',
        name,
        ...identity,
        status: 'failed',
      });
      throw new HarnessInfrastructureError('tool', 'tool_failed');
    }
    world = execution.world;
    await emit({
      type: 'tool.call.finished',
      name,
      ...identity,
      output: execution.output,
      evidenceHash: artifactHash({ name, input, output: execution.output }),
    });
    await crash('after-tool');
    return execution.output;
  };

  let result: EngineResult | undefined;
  let infrastructure: ReturnType<typeof failureDetails> | undefined;
  try {
    await crash('before-prepare');
    await emit({
      type: 'run.started',
      runId: run.id,
      conditionId: condition.id,
      caseId: benchmarkCase.id,
    });
    result = await dependencies.execute({
      run,
      benchmarkCase,
      condition,
      policy: boundaryPolicy(condition),
      emit,
      startModelCall,
      callTool,
      world: () => world,
    });
  } catch (error) {
    if (error instanceof ModelCallBudgetError) {
      result = {
        status: 'failed',
        outcome: { failureCode: 'model_call_budget', modelCalls },
        usage: { inputTokens: 0, outputTokens: 0, costUsd: 0 },
      };
    } else {
      infrastructure = failureDetails(error);
    }
  }
  try {
    await crash('before-terminal');
  } catch (error) {
    infrastructure ??= failureDetails(error);
  }
  const finished = clock();
  const status =
    infrastructure === undefined
      ? (result?.status ?? 'failed')
      : 'infrastructure';
  await emit({
    type: 'run.finished',
    status,
    infrastructure: infrastructure?.code ?? null,
  });
  const trace = await dependencies.events.derive(run.id, attempt).catch(() => {
    throw new HarnessInfrastructureError('store', 'trace_derive_failed');
  });
  const meteredUsage = dependencies.usage?.() ?? result?.usage;
  const usage = {
    inputTokens: meteredUsage?.inputTokens ?? 0,
    outputTokens: meteredUsage?.outputTokens ?? 0,
    ...(meteredUsage?.cachedInputTokens === undefined
      ? {}
      : { cachedInputTokens: meteredUsage.cachedInputTokens }),
    ...(meteredUsage?.reasoningTokens === undefined
      ? {}
      : { reasoningTokens: meteredUsage.reasoningTokens }),
    modelCalls,
    toolCalls,
    costUsd: meteredUsage?.costUsd ?? 0,
  };
  const record = ExecutionRecordV1.parse({
    schemaVersion: 1,
    attempt,
    run,
    status,
    startedAt: iso(started),
    finishedAt: iso(finished),
    durationMs: Math.max(0, finished.getTime() - started.getTime()),
    firstModelCallStarted,
    trace,
    outcome: infrastructure === undefined ? (result?.outcome ?? null) : null,
    worldHash: worldHash(world),
    usage,
    infrastructureFailure:
      infrastructure === undefined
        ? null
        : {
            ...infrastructure,
            beforeFirstModelCall: !firstModelCallStarted,
          },
  });
  await dependencies.records.append(record).catch(() => {
    throw new HarnessInfrastructureError('store', 'record_append_failed');
  });
  await crash('after-terminal');
  return record;
};
