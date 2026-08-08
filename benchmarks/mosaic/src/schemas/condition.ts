import { z } from 'zod';

import { Id, NonNegativeInteger, SchemaVersion } from './common.js';

export const ConditionFactorsV1 = z
  .object({
    decomposition: z.enum(['none', 'task', 'goal']),
    catalogFeedback: z.boolean(),
    skillView: z.enum(['none', 'metadata', 'body']),
    retrieval: z.enum(['none', 'top-k', 'oracle']),
    maxSkills: NonNegativeInteger.nullable(),
    bundle: z.enum(['none', 'single', 'selective', 'top-k', 'oracle']),
    bundleOrder: z.enum(['not-applicable', 'ranked', 'shuffled']),
    menu: z.enum(['global', 'declared', 'base-plus-bundle', 'oracle']),
    baseTools: z.boolean(),
    localizedRevision: z.boolean(),
  })
  .strict();

/** One frozen treatment whose factors make ablation differences auditable. */
export const ConditionV1 = z
  .object({
    schemaVersion: SchemaVersion,
    id: Id,
    label: z.string().trim().min(1),
    description: z.string().trim().min(1),
    kind: z.enum(['baseline', 'mosaic', 'ablation', 'oracle', 'smoke']),
    eligibility: z.enum(['all', 'failures-only', 'opt-in']),
    factors: ConditionFactorsV1,
    declaredChange: z.string().trim().min(1),
    oracleFor: Id.optional(),
  })
  .strict();

export type Condition = z.infer<typeof ConditionV1>;
