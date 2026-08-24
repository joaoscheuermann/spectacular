import * as z from 'zod';

import type { OrderedBundle, SkillCandidate } from '../types/routing.js';

const NameSchema = z.string().trim().min(1);
const RationaleSchema = z.string().trim().min(1).max(500);

export const SkillCandidateSchema = z
  .object({
    skillName: NameSchema,
    score: z.number().finite(),
    rank: z.number().int().positive(),
    rationale: RationaleSchema,
  })
  .strict() satisfies z.ZodType<SkillCandidate>;

export const OrderedBundleSchema = z
  .object({
    goalId: NameSchema,
    skills: z.array(NameSchema),
    selectionRationale: RationaleSchema,
  })
  .strict()
  .superRefine((bundle, context) => {
    if (new Set(bundle.skills).size === bundle.skills.length) return;
    context.addIssue({
      code: 'custom',
      path: ['skills'],
      message: 'Bundle skills must be unique.',
    });
  }) satisfies z.ZodType<OrderedBundle>;

type RoutingTrace = {
  readonly id: string;
  readonly candidates: readonly SkillCandidate[];
  readonly bundle: OrderedBundle | null;
};

/** Validates relationships that span the candidate trace and selected bundle. */
export const validateRoutingTrace = (
  node: RoutingTrace,
  context: z.RefinementCtx,
  prefix: readonly PropertyKey[] = [],
): void => {
  const names = node.candidates.map(({ skillName }) => skillName);

  node.candidates.forEach(({ rank }, index) => {
    if (rank === index + 1) return;
    context.addIssue({
      code: 'custom',
      path: [...prefix, 'candidates', index, 'rank'],
      message: `Candidate rank must be ${index + 1}.`,
    });
  });

  if (new Set(names).size !== names.length) {
    context.addIssue({
      code: 'custom',
      path: [...prefix, 'candidates'],
      message: 'Candidate skill names must be unique.',
    });
  }

  if (node.bundle === null) {
    if (node.candidates.length === 0) return;
    context.addIssue({
      code: 'custom',
      path: [...prefix, 'bundle'],
      message: 'Candidates require a materialized bundle.',
    });
    return;
  }

  if (node.bundle.goalId !== node.id) {
    context.addIssue({
      code: 'custom',
      path: [...prefix, 'bundle', 'goalId'],
      message: `Bundle goalId must be ${node.id}.`,
    });
  }

  const selected = new Set(node.bundle.skills);
  const ordered = names.filter((name) => selected.has(name));
  if (sameItems(node.bundle.skills, ordered)) return;
  context.addIssue({
    code: 'custom',
    path: [...prefix, 'bundle', 'skills'],
    message: 'Bundle skills must be an ordered subset of candidates.',
  });
};

const sameItems = (
  left: readonly string[],
  right: readonly string[],
): boolean =>
  left.length === right.length &&
  left.every((item, index) => item === right[index]);
