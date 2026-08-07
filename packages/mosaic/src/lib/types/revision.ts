import type * as z from 'zod';

import type {
  ObservationSchema,
  RevisionRequestSchema,
} from '../schemas/revision.js';

export type Observation = z.output<typeof ObservationSchema>;
export type RevisionRequest = z.output<typeof RevisionRequestSchema>;
