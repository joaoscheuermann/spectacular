import { z } from 'zod';

export const BundleCandidateDecisionSchema = z.object({
  name: z.string().trim().min(1),

  decision: z.enum([
    'select',
    'reject:irrelevant',
    'reject:unnecessary',
    'reject:redundant',
    'reject:conflicting',
    'reject:limit',
  ]),

  contributesTo: z.array(z.string()).describe(
    'Completion criteria from doneWhen that the skill helps satisfy.',
  ),

  redundantWith: z.array(z.string()).describe(
    'Selected candidate skills that already provide substantially the same behavior.',
  ),

  conflictsWith: z.array(z.string()).describe(
    'Selected candidate skills whose instructions are incompatible with this skill.',
  ),

  rationale: z.string().trim().min(1).max(500),
}).strict();

export const BundleSchema = z.object({
  goal: z.string(),

  skills: z.array(z.string()).describe(
    'Selected skills in the same relative order as the reranked candidates.',
  ),

  decisions: z.array(BundleCandidateDecisionSchema),

  rationale: z.string().trim().min(1).max(1000),
}).strict();
