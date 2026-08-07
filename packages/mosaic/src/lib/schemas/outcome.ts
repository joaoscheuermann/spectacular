import { z } from 'zod';

import type { Node } from '../types/graph.js';
import { RevisionRequestSchema } from './revision.js';

/**
 * Records one ordered evaluation from `doneWhen`. Section 4.8 and Appendix A,
 * table A.2 require completion to satisfy `doneWhen`; exposing every evaluation
 * is a stricter Doric proof field rather than a field mandated by the paper.
 */
const CriterionSchema = z
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
 * Carries additional result material. The paper's FinalDelivery contract in
 * Appendix A, table A.2 includes artifactRefs; this MIME/data shape is Doric's
 * existing concrete artifact representation.
 */
const ArtifactSchema = z
  .object({
    mime: z.string().min(1).describe('MIME type of the artifact.'),
    data: z.string().describe('Artifact content or reference.'),
  })
  .strict();

/**
 * Promotes Markdown plus optional artifacts only from completed outcomes.
 * Table A.2 states that a completed NodeOutcome requires a result satisfying
 * `doneWhen`; the Markdown serialization is local to Doric.
 */
const ResultSchema = z
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
const DecisionSchema = z
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

export type NodeDecision = z.output<typeof DecisionSchema>;

/** Creates the node-bound semantic decision schema used for model output. */
export const createNodeDecisionSchema = (node: Node) =>
  DecisionSchema.extend({
    criteria: DecisionSchema.shape.criteria.length(node.doneWhen.length),
  }).superRefine((outcome, context) => {
    validateCriteria(node, outcome, context);
    validateStatus(node, outcome, context);
  });

type Decision = z.output<typeof DecisionSchema>;
type RefinementContext = z.RefinementCtx;

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
  node: Node,
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
    validateRevision(node, outcome, context);
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
  node: Node,
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

  if (request.goalId !== node.id) {
    context.addIssue({
      code: 'custom',
      path: ['revisionRequest', 'goalId'],
      message: `Revision goalId must be ${node.id}.`,
    });
  }
};
