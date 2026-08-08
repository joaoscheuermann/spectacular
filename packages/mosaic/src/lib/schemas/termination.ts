import * as z from 'zod';

import { ObservationSchema } from './revision.js';

const BlockedTerminationSchema = z.object({ status: z.literal('blocked') });

/** Runtime-owned causes that terminate a node without inventing model evidence. */
export const RuntimeTerminationSchema = z.discriminatedUnion('type', [
  BlockedTerminationSchema.extend({
    type: z.literal('turn_limit'),
    limit: z.number().int().positive().safe(),
    observations: z.array(ObservationSchema),
  }).strict(),
  BlockedTerminationSchema.extend({
    type: z.literal('revision_limit'),
    limit: z.number().int().nonnegative().safe(),
  }).strict(),
  BlockedTerminationSchema.extend({
    type: z.literal('dependency'),
    dependencyIds: z
      .array(z.string().trim().min(1))
      .min(1)
      .superRefine((ids, context) => {
        if (new Set(ids).size === ids.length) return;
        context.addIssue({
          code: 'custom',
          message: 'Dependency IDs must be unique.',
        });
      }),
  }).strict(),
]);

export type RuntimeTermination = z.output<typeof RuntimeTerminationSchema>;
