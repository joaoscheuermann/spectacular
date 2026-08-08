import * as z from 'zod';

import { FinalDeliverySchema } from './delivery.js';
import { NodeOutcomeSchema } from './outcome.js';
import { RuntimeTerminationSchema } from './termination.js';
import {
  OrderedBundleSchema,
  SkillCandidateSchema,
  validateRoutingTrace,
} from './routing.js';

export const WorkflowNodeResultSchema = z
  .object({
    id: z.string().trim().min(1),
    goal: z.string().trim().min(1),
    doneWhen: z.array(z.string().trim().min(1)).min(1),
    status: z.enum(['completed', 'blocked', 'failed']),
    candidates: z.array(SkillCandidateSchema),
    bundle: OrderedBundleSchema.nullable(),
    outcome: NodeOutcomeSchema.nullable(),
    termination: RuntimeTerminationSchema.nullable(),
  })
  .strict()
  .superRefine((node, context) => {
    validateRoutingTrace(node, context);
    validateNodeResult(node, context);
  });

export const MosaicResultSchema = z.discriminatedUnion('status', [
  z
    .object({
      status: z.literal('completed'),
      delivery: FinalDeliverySchema,
      nodes: z.array(WorkflowNodeResultSchema).min(1),
    })
    .strict()
    .superRefine((result, context) => {
      if (result.nodes.every(({ status }) => status === 'completed')) return;
      context.addIssue({
        code: 'custom',
        path: ['nodes'],
        message: 'Completed workflow requires every node to be completed.',
      });
    }),
  z
    .object({
      status: z.enum(['blocked', 'failed']),
      nodes: z.array(WorkflowNodeResultSchema).min(1),
    })
    .strict()
    .superRefine((result, context) => {
      const hasFailed = result.nodes.some(({ status }) => status === 'failed');
      const hasBlocked = result.nodes.some(
        ({ status }) => status === 'blocked',
      );
      const valid =
        result.status === 'failed' ? hasFailed : hasBlocked && !hasFailed;
      if (valid) return;
      context.addIssue({
        code: 'custom',
        path: ['nodes'],
        message: 'Workflow status must match terminal node precedence.',
      });
    }),
]);

type NodeResult = z.input<typeof WorkflowNodeResultSchema>;

const validateNodeResult = (
  node: NodeResult,
  context: z.RefinementCtx,
): void => {
  const observations = [
    ...(node.outcome?.observations ?? []),
    ...(node.termination?.type === 'turn_limit'
      ? node.termination.observations
      : []),
  ];
  if (
    observations.some(({ goalId }) => goalId !== node.id) ||
    (node.outcome?.revisionRequest !== null &&
      node.outcome?.revisionRequest !== undefined &&
      node.outcome.revisionRequest.goalId !== node.id)
  ) {
    context.addIssue({
      code: 'custom',
      message: 'Outcome evidence must belong to the result node.',
    });
  }

  if (node.status === 'completed') {
    requireOutcome(node, 'completed', context);
    return;
  }

  if (node.status === 'failed') {
    requireOutcome(node, 'failed', context);
    return;
  }

  const modelBlocked =
    node.outcome?.status === 'blocked' && node.termination === null;
  const turnBlocked =
    node.outcome === null && node.termination?.type === 'turn_limit';
  const dependencyBlocked =
    node.outcome === null && node.termination?.type === 'dependency';
  const revisionBlocked =
    node.outcome?.status === 'needs_revision' &&
    node.termination?.type === 'revision_limit';

  if (modelBlocked || turnBlocked || dependencyBlocked || revisionBlocked)
    return;
  context.addIssue({
    code: 'custom',
    message: 'Blocked node outcome and termination are inconsistent.',
  });
};

const requireOutcome = (
  node: NodeResult,
  status: 'completed' | 'failed',
  context: z.RefinementCtx,
): void => {
  if (node.outcome?.status === status && node.termination === null) return;
  context.addIssue({
    code: 'custom',
    message: `${status} node requires a matching outcome and no termination.`,
  });
};
