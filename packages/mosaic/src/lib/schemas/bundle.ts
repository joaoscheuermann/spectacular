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
      goalId: z.literal(goalId),
      evaluations: z
        .array(
          z
            .object({
              skillName: z.enum(names),
              selected: z.boolean(),
              rationale: z.string().trim().min(1).max(500),
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
        }),
      selectionRationale: z.string().trim().min(1).max(500),
    })
    .strict();
};
