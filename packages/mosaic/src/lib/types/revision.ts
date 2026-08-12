import type * as z from 'zod';

import type { RevisionRequestSchema } from '../schemas/revision.js';

export type RevisionRequest = z.output<typeof RevisionRequestSchema>;
