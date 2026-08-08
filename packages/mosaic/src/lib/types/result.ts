import type { FinalDelivery } from './delivery.js';
import type { NodeOutcome } from '../schemas/outcome.js';
import type { RuntimeTermination } from '../schemas/termination.js';

export interface WorkflowNodeResult {
  readonly id: string;
  readonly goal: string;
  readonly doneWhen: readonly string[];
  readonly status: 'completed' | 'blocked' | 'failed';
  readonly outcome: NodeOutcome | null;
  readonly termination: RuntimeTermination | null;
}

export type MosaicResult =
  | {
      readonly status: 'completed';
      readonly delivery: FinalDelivery;
      readonly nodes: readonly WorkflowNodeResult[];
    }
  | {
      readonly status: 'blocked' | 'failed';
      readonly nodes: readonly WorkflowNodeResult[];
    };
