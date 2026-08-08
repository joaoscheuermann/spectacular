import type * as z from 'zod';

import { NodeSchema, GraphSchema } from '../schemas/graph.js';
import type { NodeOutcome } from '../schemas/outcome.js';
import type { RuntimeTermination } from '../schemas/termination.js';

export interface NodeRuntimeState {
  readonly outcome: NodeOutcome | null;
  readonly termination: RuntimeTermination | null;
}

export type Node = z.output<typeof NodeSchema>;
export type Graph = z.output<typeof GraphSchema>;
