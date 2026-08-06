import { z } from 'zod';

/** JSON-Schema-compatible representation of a bundle skill. */
export const SkillSchema = z.object({
  name: z.string(),
  description: z.string(),
  body: z.string(),
  allowedTools: z.array(z.string()),
});
