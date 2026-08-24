import * as z from 'zod';

/** Creates the node-bound structured contract for one skill selection. */
export const createBundleSelectionSchema = (
  goalId: string,
  candidates: readonly string[],
  maxSkills: number,
) => {
  const names = candidates as [string, ...string[]];

  return z
    .object({
      goalId: z
        .literal(goalId)
        .describe(
          "Required constant for the current node. Copy this field's `const` value exactly, including its complete prefix and suffix. Do not infer, shorten, rewrite, or substitute another node ID.",
        ),
      evaluations: z
        .array(
          z
            .object({
              skillName: z
                .enum(names)
                .describe(
                  "Required candidate name. Copy one of this field's `enum` values exactly. Do not infer, shorten, rewrite, or invent a skill name. Use each allowed value exactly once across `evaluations`.",
                ),
              selected: z
                .boolean()
                .describe(
                  'Whether this exact candidate belongs to the smallest sufficient skill set.',
                ),
              rationale: z
                .string()
                .trim()
                .min(1)
                .max(500)
                .describe(
                  'Concise reason this exact candidate is selected or rejected for the current node.',
                ),
            })
            .strict(),
        )
        .length(candidates.length)
        .superRefine((evaluations, ctx) => {
          const seen = new Set<string>();

          evaluations.forEach(({ skillName }, index) => {
            if (!seen.has(skillName)) {
              seen.add(skillName);
              return;
            }

            ctx.addIssue({
              code: 'custom',
              message: `Duplicate candidate evaluation: ${skillName}`,
              path: [index, 'skillName'],
            });
          });

          if (seen.size !== candidates.length) {
            ctx.addIssue({
              code: 'custom',
              message: 'Every candidate must be evaluated exactly once.',
            });
          }

          if (
            evaluations.filter(({ selected }) => selected).length > maxSkills
          ) {
            ctx.addIssue({
              code: 'custom',
              message: `At most ${maxSkills} candidates may be selected.`,
            });
          }
        })
        .describe(
          `Exactly ${candidates.length} candidate evaluations. Include every allowed \`skillName\` exactly once; do not omit, duplicate, rename, or add candidates. At most ${maxSkills} ${maxSkills === 1 ? 'evaluation' : 'evaluations'} may set \`selected\` to true.`,
        ),
      selectionRationale: z
        .string()
        .trim()
        .min(1)
        .max(500)
        .describe(
          'Concise reason the selected candidates form the smallest sufficient skill set for the current node.',
        ),
    })
    .strict()
    .describe(
      'Complete skill-selection decision for exactly one current node and its complete candidate menu.',
    );
};
