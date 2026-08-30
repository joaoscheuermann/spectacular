import * as z from 'zod';

export const caseSchema = z
  .object({
    name: z.string().trim().min(1),
    objective: z.string().trim().min(1),
    skills: z
      .object({ expected: z.array(z.string().trim().min(1)).min(1) })
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
