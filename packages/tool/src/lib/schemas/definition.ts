import { z } from 'zod';

import { JsonObjectSchema } from './json.js';

/** JSON-Schema-compatible representation of provider-facing tool metadata. */
export const ToolDefinitionSchema = z.object({
  name: z.string(),
  description: z.string().optional(),
  inputSchema: JsonObjectSchema,
  strict: z.boolean().optional(),
});
