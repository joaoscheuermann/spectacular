import * as z from 'zod';

export const SkillHintEffectSchema = z
  .enum(['vocabulary', 'gap', 'division', 'dependency'])
  .describe(
    'The strongest material effect of the skill body on the current goal: ' +
      'clarifying required terminology, revealing an omitted observable result, ' +
      'showing an unnecessary split with a directly related goal, or identifying ' +
      'an incorrect dependency involving the current goal.',
  );

/**
 * A body-grounded hint that materially affects the definition or
 * local structure of the current goal.
 */
export const SkillHintSchema = z
  .object({
    effect: SkillHintEffectSchema,

    evidence: z
      .string()
      .trim()
      .min(1)
      .max(500)
      .describe(
        'A concise explanation grounded in the skill body. State what the body ' +
          'establishes and why it materially changes the current goal. For an ' +
          'artificial split or dependency, identify the directly related goal IDs. ' +
          'Do not describe execution steps or general skill applicability.',
      ),
  })
  .strict();

/**
 * Structured output of the hint-extraction call.
 *
 * `null` means that the skill body reveals no concrete issue or
 * material clarification affecting the current goal.
 */
export const SkillHintExtractionSchema = z
  .object({
    hints: z.array(SkillHintSchema).describe(''),
  })
  .strict();
