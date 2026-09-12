import type * as z from 'zod';

import { GraphSchema,NodeSchema } from '../schemas/graph.js';
import type { Observation } from '../schemas/observation.js';
import type { NodeOutcome } from '../schemas/outcome.js';
import type { RuntimeTermination } from '../schemas/termination.js';
import type { OrderedBundle, SkillCandidate } from './routing.js';

export interface NodeRuntimeState {
  readonly candidates: readonly SkillCandidate[];
  readonly bundle: OrderedBundle | null;
  readonly observations: readonly Observation[];
  readonly outcome: NodeOutcome | null;
  readonly termination: RuntimeTermination | null;
}

export type Node = z.output<typeof NodeSchema>;

export type Graph = z.output<typeof GraphSchema>;
