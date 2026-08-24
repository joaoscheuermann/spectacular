import type * as z from 'zod';

import type { RevisionRequestSchema } from '../schemas/revision.js';

export type RevisionRequest = z.output<typeof RevisionRequestSchema>;

/** Historical context carried from one successful localized revision. */
export type RevisionExecutionHandoff = {
  readonly invalidatedAssumption: string;
  readonly requestedEffect: string;
  readonly falseCriteria: readonly {
    readonly criterionIndex: number;
    readonly text: string;
  }[];
  readonly observations: readonly {
    readonly toolName: string;
    readonly input: string;
    readonly output: string;
  }[];
  readonly omittedObservationCount: number;
};
