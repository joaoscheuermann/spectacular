import * as z from 'zod';

import { complete, type CompletionConfig } from '../../agent.js';
import { CLASSIFICATION_MAX_OUTPUT_TOKENS } from '../../constants.js';
import { KINDS } from './kinds.js';

export const schema = z
  .object({
    type: z.enum(KINDS),
  })
  .strict();

export type Categorization = z.infer<typeof schema>;

export const classify = (
  config: CompletionConfig,
  system: string,
  path: string,
  content: string,
): Promise<Categorization> =>
  complete({
    ...config,
    system,
    input: JSON.stringify({ path, content }),
    schema,
    stage: `Classification for ${path}`,
    maxOutputTokens: CLASSIFICATION_MAX_OUTPUT_TOKENS,
  });

export { KINDS, type Kind } from './kinds.js';
