import * as z from 'zod';

export const ObservationSchema = z
  .object({
    goalId: z
      .string()
      .trim()
      .min(1)
      .describe('ID of the node that produced this runtime-owned observation.'),
    toolName: z
      .string()
      .trim()
      .min(1)
      .describe('Name of the executable tool that produced this observation.'),
    callId: z
      .string()
      .trim()
      .min(1)
      .describe(
        'Runtime-owned tool-call identifier used only for correlation.',
      ),
    input: z
      .string()
      .describe('Serialized input passed to the executable tool.'),
    output: z
      .string()
      .describe('Serialized result returned by the executable tool.'),
  })
  .strict();

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
