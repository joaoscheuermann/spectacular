import * as z from 'zod';

/** Describes the semantic plan change requested by an executing node. */
export const RevisionRequestSchema = z
  .object({
    goalId: z
      .string()
      .trim()
      .min(1)
      .describe(
        'Exact ID of the current node whose plan must be revised; it must match the Current Node ID.',
      ),
    invalidatedAssumption: z
      .string()
      .trim()
      .min(1)
      .describe(
        'Specific structural planning assumption disproved by a current-node tool observation.',
      ),
    requestedEffect: z
      .string()
      .trim()
      .min(1)
      .describe(
        'Specific structural change the localized planner must make so execution can continue.',
      ),
  })
  .strict();
