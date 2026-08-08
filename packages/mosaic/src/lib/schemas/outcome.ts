import { z } from 'zod';

import type { Node } from '../types/graph.js';
import { ArtifactSchema } from './artifact.js';
import { RevisionRequestSchema } from './revision.js';
import { ObservationSchema } from './revision.js';

/**
 * Records one ordered evaluation from `doneWhen`. Section 4.9 and Appendix A,
 * table A.2 require completion to satisfy `doneWhen`; exposing every evaluation
 * is a stricter Doric proof field rather than a field mandated by the paper.
 */
export const CriterionSchema = z
  .object({
    criterionIndex: z
      .number()
      .int()
      .nonnegative()
      .describe('Zero-based index of the evaluated doneWhen criterion.'),
    satisfied: z
      .boolean()
      .describe(
        'Whether the criterion is satisfied by the available evidence.',
      ),
    evidence: z
      .string()
      .trim()
      .min(1)
      .describe('Concise evidence supporting this criterion evaluation.'),
  })
  .strict();

/**
 * Promotes Markdown plus optional artifacts only from completed outcomes.
 * Table A.2 states that a completed NodeOutcome requires a result satisfying
 * `doneWhen`; the Markdown serialization is local to Doric.
 */
export const ResultSchema = z
  .object({
    markdown: z
      .string()
      .min(1)
      .describe('The node result in the language of the original request.'),
    artifacts: z
      .array(ArtifactSchema)
      .describe('Additional artifacts produced by the node.'),
  })
  .strict();

/**
 * Defines the semantic decision authored by an executing model. MOSAIC 0.2
 * keeps observations runtime-owned and outside this model-facing contract.
 */
const DecisionFieldsSchema = z
  .object({
    status: z.enum(['completed', 'needs_revision', 'blocked', 'failed']),
    criteria: z
      .array(CriterionSchema)
      .describe('One evaluation per doneWhen criterion in its original order.'),
    result: ResultSchema.nullable(),
    revisionRequest: RevisionRequestSchema.nullable(),
    reason: z
      .string()
      .trim()
      .min(1)
      .nullable()
      .describe('Failure or non-completion reason; null only when completed.'),
  })
  .strict();

type Decision = z.output<typeof DecisionFieldsSchema>;
type RefinementContext = z.RefinementCtx;

/** Strict semantic decision contract authored by the executing model. */
export const NodeDecisionSchema = DecisionFieldsSchema.superRefine(
  (decision, context) => validateStatus(undefined, decision, context),
);

export type NodeDecision = z.output<typeof NodeDecisionSchema>;

/** Complete runtime outcome: the model decision plus ordered observations. */
export const NodeOutcomeSchema = DecisionFieldsSchema.extend({
  observations: z.array(ObservationSchema),
}).superRefine((outcome, context) => {
  validateStatus(undefined, outcome, context);
  if (
    outcome.status === 'needs_revision' &&
    outcome.observations.length === 0
  ) {
    context.addIssue({
      code: 'custom',
      path: ['observations'],
      message: 'A needs_revision outcome requires at least one observation.',
    });
  }
});

export type NodeOutcome = z.output<typeof NodeOutcomeSchema>;

/** Creates the node-bound semantic decision schema used for model output. */
export const createNodeDecisionSchema = (node: Node) =>
  DecisionFieldsSchema.extend({
    criteria: DecisionFieldsSchema.shape.criteria.length(node.doneWhen.length),
  }).superRefine((outcome, context) => {
    validateCriteria(node, outcome, context);
    validateStatus(node.id, outcome, context);
  });

/** Validates a materialized outcome against its owning node. */
export const createNodeOutcomeSchema = (node: Node) =>
  NodeOutcomeSchema.superRefine((outcome, context) => {
    validateCriteria(node, outcome, context);
    if (
      outcome.status === 'needs_revision' &&
      outcome.revisionRequest?.goalId !== node.id
    ) {
      context.addIssue({
        code: 'custom',
        path: ['revisionRequest', 'goalId'],
        message: `Revision goalId must be ${node.id}.`,
      });
    }
  });

const validateCriteria = (
  node: Node,
  outcome: Decision,
  context: RefinementContext,
): void => {
  /** The model must evaluate every criterion exactly once and in source order. */
  if (outcome.criteria.length !== node.doneWhen.length) {
    context.addIssue({
      code: 'custom',
      path: ['criteria'],
      message: 'Criteria must contain exactly one entry per doneWhen item.',
    });
  }

  outcome.criteria.forEach((criterion, index) => {
    if (criterion.criterionIndex === index) return;

    context.addIssue({
      code: 'custom',
      path: ['criteria', index, 'criterionIndex'],
      message: `criterionIndex must be ${index}.`,
    });
  });
};

const validateStatus = (
  nodeId: string | undefined,
  outcome: Decision,
  context: RefinementContext,
): void => {
  /** Each terminal status owns a distinct result/revision/reason combination. */
  if (outcome.status === 'completed') {
    validateCompleted(outcome, context);
    return;
  }

  if (outcome.reason === null) {
    context.addIssue({
      code: 'custom',
      path: ['reason'],
      message: 'A non-completed outcome requires a reason.',
    });
  }

  if (outcome.status === 'needs_revision') {
    validateRevision(nodeId, outcome, context);
    return;
  }

  if (outcome.result !== null) {
    context.addIssue({
      code: 'custom',
      path: ['result'],
      message: `${outcome.status} outcomes cannot promote a result.`,
    });
  }

  if (outcome.revisionRequest !== null) {
    context.addIssue({
      code: 'custom',
      path: ['revisionRequest'],
      message: 'Only needs_revision may include a revision request.',
    });
  }
};

const validateCompleted = (
  outcome: Decision,
  context: RefinementContext,
): void => {
  /** Completion is the only status that promotes a fully satisfied result. */
  if (outcome.criteria.some(({ satisfied }) => !satisfied)) {
    context.addIssue({
      code: 'custom',
      path: ['criteria'],
      message: 'A completed outcome requires every criterion to be satisfied.',
    });
  }

  if (outcome.result === null) {
    context.addIssue({
      code: 'custom',
      path: ['result'],
      message: 'A completed outcome requires a result.',
    });
  }

  if (outcome.revisionRequest !== null) {
    context.addIssue({
      code: 'custom',
      path: ['revisionRequest'],
      message: 'A completed outcome cannot request a revision.',
    });
  }

  if (outcome.reason !== null) {
    context.addIssue({
      code: 'custom',
      path: ['reason'],
      message: 'A completed outcome must have a null reason.',
    });
  }
};

const validateRevision = (
  nodeId: string | undefined,
  outcome: Decision,
  context: RefinementContext,
): void => {
  /** The runtime associates the request with every observation from this node. */
  const request = outcome.revisionRequest;

  if (request === null) {
    context.addIssue({
      code: 'custom',
      path: ['revisionRequest'],
      message: 'A needs_revision outcome requires a revision request.',
    });
    return;
  }

  if (nodeId !== undefined && request.goalId !== nodeId) {
    context.addIssue({
      code: 'custom',
      path: ['revisionRequest', 'goalId'],
      message: `Revision goalId must be ${nodeId}.`,
    });
  }
};
