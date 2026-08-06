import { z } from 'zod';

/** JSON-Schema-compatible tool metadata for strict structured outputs. */
export const ToolMetadataSchema = z.object({
  name: z.string(),
  description: z.string(),
});
