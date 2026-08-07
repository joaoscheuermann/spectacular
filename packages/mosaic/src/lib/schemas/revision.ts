import * as z from 'zod';

export const ObservationSchema = z
  .object({
    goalId: z.string().trim().min(1),
    toolName: z.string().trim().min(1),
    callId: z.string().trim().min(1),
    input: z.string(),
    output: z.string(),
  })
  .strict();

/** Describes the semantic plan change requested by an executing node. */
export const RevisionRequestSchema = z
  .object({
    goalId: z.string().trim().min(1),
    invalidatedAssumption: z.string().trim().min(1),
    requestedEffect: z.string().trim().min(1),
  })
  .strict();
