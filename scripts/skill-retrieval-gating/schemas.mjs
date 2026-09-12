import * as z from 'zod';

const skillName = z.string().trim().min(1);

const uniqueNames = (schema) =>
  schema.refine((names) => new Set(names).size === names.length, {
    message: 'Skill names must be unique.',
  });
const expectedSchema = uniqueNames(z.array(skillName).min(1));
const usefulSchema = uniqueNames(z.array(skillName));

export const noiseTypeSchema = z.enum([
  'irrelevant',
  'no_operational_value',
  'conflicting',
]);

const noiseSchema = z.record(
  skillName,
  z.object({ type: noiseTypeSchema }).strict(),
);

export const caseSchema = z
  .object({
    name: z.string().trim().min(1),
    objective: z.string().trim().min(1),
    skills: z
      .object({
        expected: expectedSchema,
        useful: usefulSchema,
        noise: noiseSchema,
      })
      .superRefine(({ expected, useful, noise }, context) => {
        const owners = new Map();

        const categories = [
          ['expected', expected],
          ['useful', useful],
          ['noise', Object.keys(noise)],
        ];

        for (const [category, names] of categories) {
          for (const name of names) {
            const owner = owners.get(name);

            if (!owner) {
              owners.set(name, category);

              continue;
            }

            context.addIssue({
              code: 'custom',
              message: `${name} appears in both ${owner} and ${category}.`,
              path: [category],
            });
          }
        }
      })
      .strict(),
  })
  .strict();

export const goalsSchema = z
  .object({ goals: z.array(z.string().trim().min(1)).min(1) })
  .strict();

export const gateSchema = z
  .object({
    decision: z.enum(['keep', 'drop']),
    reason: z.string().trim().min(1),
  })
  .strict();
