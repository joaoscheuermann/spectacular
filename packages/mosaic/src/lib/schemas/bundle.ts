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
      skills: z
        .array(
          z
            .object({
              skill: z.enum(names),
              rationale: z.string().trim().min(1).max(500),
            })
            .strict(),
        )
        .max(maxSkills)
        .superRefine((skills, ctx) => {
          const seen = new Set<string>();

          skills.forEach(({ skill }, index) => {
            if (!seen.has(skill)) {
              seen.add(skill);
              return;
            }

            ctx.addIssue({
              code: 'custom',
              message: `Duplicate selected skill: ${skill}`,
              path: [index, 'skill'],
            });
          });
        }),
    })
    .strict();
};
