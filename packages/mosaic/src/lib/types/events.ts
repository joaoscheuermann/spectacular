export type MosaicCapture = 'structure' | 'io';

export type MosaicStage =
  | 'run'
  | 'plan'
  | 'schedule'
  | 'bundle'
  | 'execution'
  | 'revision'
  | 'delivery';

type EventBase = {
  readonly schemaVersion: 3;
  readonly runId: string;
  readonly sequence: number;
};

type LifecycleEvent =
  | {
      readonly type: 'run.started';
      readonly stage: 'run';
      readonly input?: string;
    }
  | {
      readonly type: 'run.finished';
      readonly stage: 'run';
      readonly durationMs: number;
      readonly status: 'completed' | 'blocked' | 'failed';
    }
  | {
      readonly type: 'run.failed';
      readonly stage: 'run';
      readonly durationMs: number;
    }
  | {
      readonly type: 'stage.started';
      readonly stage: Exclude<MosaicStage, 'run'>;
    }
  | {
      readonly type: 'stage.finished';
      readonly stage: Exclude<MosaicStage, 'run'>;
      readonly durationMs: number;
    }
  | {
      readonly type: 'stage.failed';
      readonly stage: Exclude<MosaicStage, 'run'>;
      readonly durationMs: number;
    };

type ModelEvent =
  | {
      readonly type: 'tool.repair';
      readonly providerId: string;
      readonly stage: MosaicStage;
      readonly nodeId?: string;
      readonly revision?: number;
      readonly attempt: number;
      readonly maxAttempts: number;
    }
  | {
      readonly type: 'structured.attempt';
      readonly providerId: string;
      readonly stage: MosaicStage;
      readonly nodeId?: string;
      readonly revision?: number;
      readonly attempt: number;
      readonly runtimeAccepted: boolean;
      readonly feedbackSent: boolean;
      readonly diagnostic?: string;
    }
  | {
      readonly type: 'model.request';
      readonly providerId: string;
      readonly stage: MosaicStage;
      readonly nodeId?: string;
      readonly revision?: number;
      readonly operation: 'complete' | 'stream' | 'rerank';
      readonly model: string;
      readonly messageCount?: number;
      readonly toolNames?: readonly string[];
      readonly content?: unknown;
    }
  | {
      readonly type: 'model.response';
      readonly providerId: string;
      readonly stage: MosaicStage;
      readonly nodeId?: string;
      readonly revision?: number;
      readonly operation: 'complete' | 'stream' | 'rerank';
      readonly model: string;
      readonly durationMs: number;
      readonly finishReason?: string;
      readonly content?: unknown;
    };

type PlanningEvent =
  | {
      readonly type: 'plan.snapshot';
      readonly stage: 'plan';
      readonly phase: 'p0' | 'p1';
      readonly revision: number;
      readonly nodeIds: readonly string[];
      readonly graph?: unknown;
    }
  | {
      readonly type: 'hint.result';
      readonly stage: 'plan';
      readonly nodeId: string;
      readonly revision: number;
      readonly skillName: string;
      readonly hintCount: number;
      readonly hints?: unknown;
    }
  | {
      readonly type: 'graph.revised';
      readonly stage: 'revision';
      readonly revision: number;
      readonly nodeId: string;
      readonly nodeIds: readonly string[];
      readonly graph?: unknown;
    };

type RoutingEvent =
  | {
      readonly type: 'retrieval.result';
      readonly stage: 'plan' | 'bundle';
      readonly nodeId: string;
      readonly revision: number;
      readonly query?: string;
      readonly skillNames: readonly string[];
    }
  | {
      readonly type: 'rerank.result';
      readonly stage: 'bundle';
      readonly nodeId: string;
      readonly revision: number;
      readonly ranking: readonly {
        readonly skillName: string;
        readonly score: number;
        readonly rank: number;
      }[];
    }
  | {
      readonly type: 'bundle.selected';
      readonly stage: 'bundle';
      readonly nodeId: string;
      readonly revision: number;
      readonly candidateNames: readonly string[];
      readonly skillNames: readonly string[];
      readonly candidates?: unknown;
      readonly bundle?: unknown;
    }
  | {
      readonly type: 'menu.composed';
      readonly stage: 'bundle';
      readonly nodeId: string;
      readonly revision: number;
      readonly toolNames: readonly string[];
    };

type ExecutionEvent =
  | {
      readonly type: 'wave.started' | 'wave.finished';
      readonly stage: 'execution';
      readonly revision: number;
      readonly nodeIds: readonly string[];
    }
  | {
      readonly type: 'node.status';
      readonly stage: 'schedule' | 'execution' | 'revision';
      readonly revision: number;
      readonly nodeId: string;
      readonly status: string;
    }
  | {
      readonly type: 'tool.started';
      readonly stage: 'execution';
      readonly nodeId: string;
      readonly revision: number;
      readonly callId: string;
      readonly toolName: string;
      readonly input?: unknown;
    }
  | {
      readonly type: 'tool.finished';
      readonly stage: 'execution';
      readonly nodeId: string;
      readonly revision: number;
      readonly callId: string;
      readonly toolName: string;
      readonly observationId: string;
      readonly durationMs: number;
      readonly output?: unknown;
    }
  | {
      readonly type: 'tool.failed';
      readonly stage: 'execution';
      readonly nodeId: string;
      readonly revision: number;
      readonly callId: string;
      readonly toolName: string;
      readonly durationMs: number;
    }
  | {
      readonly type: 'decision.created';
      readonly stage: 'execution';
      readonly nodeId: string;
      readonly revision: number;
      readonly status: string;
      readonly decision?: unknown;
    }
  | {
      readonly type: 'observations.created';
      readonly stage: 'execution';
      readonly nodeId: string;
      readonly revision: number;
      readonly count: number;
      readonly toolNames: readonly string[];
      readonly observations?: unknown;
    }
  | {
      readonly type: 'outcome.created';
      readonly stage: 'execution';
      readonly nodeId: string;
      readonly revision: number;
      readonly status: string;
      readonly outcome?: unknown;
    }
  | {
      readonly type: 'delivery.created';
      readonly stage: 'delivery';
      readonly partIds: readonly string[];
      readonly delivery?: unknown;
    };

/** Versioned, discriminated events emitted by one MOSAIC run. */
export type MosaicEvent = EventBase &
  (LifecycleEvent | ModelEvent | PlanningEvent | RoutingEvent | ExecutionEvent);

export type MosaicObserver = (event: MosaicEvent) => void | Promise<void>;

export interface MosaicRunOptions {
  readonly runId?: string;
  readonly signal?: AbortSignal;
  readonly observer?: MosaicObserver;
  readonly capture?: MosaicCapture;
}
