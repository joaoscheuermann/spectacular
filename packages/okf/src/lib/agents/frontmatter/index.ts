import * as z from 'zod';

import { complete, type CompletionConfig } from '../../agent.js';
import { SUMMARY_MAX_OUTPUT_TOKENS } from '../../constants.js';
import type { Categorization } from '../classify/index.js';

const schema = z
  .object({
    type: z.string().trim().min(1),
    title: z.string().trim().min(1),
    description: z.string().trim().min(1),
    tags: z.array(z.string().trim().min(1)),
  })
  .strict();

export type Frontmatter = z.infer<typeof schema>;

export const frontmatter = (
  config: CompletionConfig,
  system: string,
  path: string,
  content: string,
  classification: Categorization,
): Promise<Frontmatter> =>
  complete({
    ...config,
    system,
    input: JSON.stringify({
      path,
      classification: classification.type,
      content,
    }),
    schema,
    stage: `Frontmatter for ${path}`,
    maxOutputTokens: SUMMARY_MAX_OUTPUT_TOKENS,
  });
