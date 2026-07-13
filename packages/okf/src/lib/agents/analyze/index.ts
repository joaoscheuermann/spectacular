import { complete, type CompletionConfig } from '../../agent.js';
import { SUMMARY_MAX_OUTPUT_TOKENS } from '../../constants.js';
import type { Categorization } from '../classify/index.js';
import { schema, type Analysis } from './schema.js';

/** Analyzes a classified file into an evidence-only Markdown summary. */
export const analyze = (
  config: CompletionConfig,
  system: string,
  path: string,
  content: string,
  classification: Categorization,
): Promise<Analysis> =>
  complete({
    ...config,
    system,
    input: JSON.stringify({
      path,
      classification: classification.type,
      content,
    }),
    schema,
    stage: `Analysis for ${path}`,
    maxOutputTokens: SUMMARY_MAX_OUTPUT_TOKENS,
  });

export type { Analysis } from './schema.js';
