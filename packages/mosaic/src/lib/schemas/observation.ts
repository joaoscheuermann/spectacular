import * as z from 'zod';

/** Runtime-owned evidence produced by one successful executable-tool result. */
export const ObservationSchema = z
  .object({
    id: z.string().trim().min(1).describe('Opaque observation identifier.'),
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
      .describe('Provider tool-call identifier retained only for correlation.'),
    input: z
      .string()
      .describe('Serialized input passed to the executable tool.'),
    output: z
      .string()
      .describe('Serialized result returned by the executable tool.'),
  })
  .strict();

export type Observation = z.output<typeof ObservationSchema>;
