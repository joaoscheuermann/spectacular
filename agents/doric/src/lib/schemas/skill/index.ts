import * as z from 'zod';

import {CanonicalNameSchema} from '../general/index.js'


/**
 * Accepts either:
 *
 * allowedTools:
 *   - read
 *   - write
 *
 * or:
 *
 * allowedTools: "read write"
 *
 * The result is always a deduplicated string array.
 */
export const AllowedToolsSchema = z
  .preprocess((value) => {
    if (value === undefined || value === null) {
      return [];
    }

    if (typeof value === 'string') {
      const normalized = value.trim();

      return normalized.length === 0
        ? []
        : normalized.split(/\s+/);
    }

    return value;
  }, z.array(CanonicalNameSchema))
  .transform((names) => [...new Set(names)])
  .describe(
    'The exact names of tools added to the tool menu when this skill is selected. ' +
    'An empty list means the skill adds no tools. The list does not describe the ' +
    'purpose of the skill and does not require any tool to be called.'
  );

/**
 * The authored content of one skill.
 *
 * This is the object produced after parsing SKILL.md and mapping
 * the YAML key `allowed-tools` to `allowedTools`.
 */
export const SkillSchema = z
  .object({
    name: CanonicalNameSchema.describe(
      'The unique canonical name of the skill. References to this skill use this exact value.'
    ),

    description: z
      .string()
      .trim()
      .min(1, 'The skill description cannot be empty.')
      .describe(
        'A concise explanation of the behavior the skill teaches and the situations ' +
        'in which it is relevant. It is used as an initial retrieval signal.'
      ),

    body: z
      .string()
      .trim()
      .min(1, 'The skill body cannot be empty.')
      .describe(
        'The complete Markdown instructions used for reranking and execution. ' +
        'It must teach a behavior, procedure, or decision criterion rather than merely repeat tool names.'
      ),

    allowedTools: AllowedToolsSchema,
  })
  .strict();
