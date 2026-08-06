import { z } from 'zod';

import type { Node } from '../types/graph.js';

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
 * Mirrors PlanRevisionRequest from Appendix A, table A.2: goal, triggering
 * observation, invalidated assumption, and requested effect.
 */
const RevisionRequestSchema = z
  .object({
    goalId: z.string(),
    triggerObservationRef: z.string(),
    invalidatedAssumption: z.string().trim().min(1),
    requestedEffect: z.string().trim().min(1),
  })
  .strict();

/**
 * Defines the terminal results emitted by an executing node. Section 4.9
 * (pp. 16-17) defines completed, needs_revision, blocked, and failed as exits
 * from running; pending, ready, and running remain scheduler-owned states.
 */
const OutcomeSchema = z
  .object({
    status: z.enum(['completed', 'needs_revision', 'blocked', 'failed']),
    criteria: z
      .array(CriterionSchema)
      .describe('One evaluation per doneWhen criterion in its original order.'),
    result: ResultSchema.nullable(),
    observationRefs: z
      .array(z.string())
      .describe('Unique IDs of tool calls used as evidence.'),
    revisionRequest: RevisionRequestSchema.nullable(),
    reason: z
      .string()
      .trim()
      .min(1)
      .nullable()
      .describe('Failure or non-completion reason; null only when completed.'),
  })
  .strict();

/**
 * Creates the node-bound form of the paper's NodeOutcome contract (Appendix A,
 * table A.2), augmented with Doric's explicit per-criterion proof entries.
 */
export const createNodeOutcomeSchema = (node: Node) =>
  OutcomeSchema.extend({
    criteria: OutcomeSchema.shape.criteria.length(node.doneWhen.length),
  }).superRefine((outcome, context) => {
    validateCriteria(node, outcome, context);
    validateObservationRefs(outcome, context);
    validateStatus(node, outcome, context);
  });

type Outcome = z.output<typeof OutcomeSchema>;
type RefinementContext = z.RefinementCtx;

const validateCriteria = (
  node: Node,
  outcome: Outcome,
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

const validateObservationRefs = (
  outcome: Outcome,
  context: RefinementContext,
): void => {
  /** Duplicate IDs cannot provide independent evidence and are rejected. */
  if (
    new Set(outcome.observationRefs).size !== outcome.observationRefs.length
  ) {
    context.addIssue({
      code: 'custom',
      path: ['observationRefs'],
      message: 'Observation references must be unique.',
    });
  }
};

const validateStatus = (
  node: Node,
  outcome: Outcome,
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
  outcome: Outcome,
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
  outcome: Outcome,
  context: RefinementContext,
): void => {
  /**
   * Revision remains local and cites an observed trigger, matching Appendix A,
   * table A.2 and the localized-revision rules in section 4.9.
   */
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

  if (!outcome.observationRefs.includes(request.triggerObservationRef)) {
    context.addIssue({
      code: 'custom',
      path: ['revisionRequest', 'triggerObservationRef'],
      message: 'Revision trigger must reference an included observation.',
    });
  }
};
