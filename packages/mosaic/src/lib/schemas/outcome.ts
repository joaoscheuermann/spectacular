import { z } from 'zod';

import { authorizedObservationIds } from '../observations.js';
import type { Graph, Node } from '../types/graph.js';
import { ArtifactSchema } from './artifact.js';
import { RevisionRequestSchema } from './revision.js';

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
      .describe(
        "Exact zero-based position in the current node's doneWhen array; the first criteria entry must use 0, the second 1, and so on in source order.",
      ),
    satisfied: z
      .boolean()
      .describe(
        'True only when the available request, deterministic result, or cited local or projected ancestor observations prove this criterion; completed requires true for every entry.',
      ),
    evidence: z
      .string()
      .trim()
      .min(1)
      .describe(
        'Non-empty concise proof for this criterion evaluation; state what establishes the decision and use observationIds separately for cited tool results.',
      ),
    observationIds: z
      .array(z.string().trim().min(1))
      .superRefine((ids, context) => {
        const seen = new Set<string>();
        ids.forEach((id, position) => {
          if (!seen.has(id)) {
            seen.add(id);
            return;
          }
          context.addIssue({
            code: 'custom',
            path: [position],
            message: 'Observation IDs must be unique within one criterion.',
          });
        });
      })
      .describe(
        'Smallest unique list of opaque observation IDs from this node or the projected ancestor evidence. Use [] when proof requires no tool result.',
      ),
  })
  .strict();

type Criterion = z.output<typeof CriterionSchema>;

export type CriterionEvaluation = Readonly<
  Omit<Criterion, 'observationIds'> & {
    readonly observationIds: readonly string[];
  }
>;

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
      .describe(
        'Non-empty final result produced by the current node, written in the language of the original request; provide it only with completed status.',
      ),
    artifacts: z
      .array(ArtifactSchema)
      .describe(
        'Ordered additional artifacts produced by the current node; use [] when there are none.',
      ),
  })
  .strict();

/**
 * Defines the semantic decision authored by an executing model. MOSAIC 0.2
 * keeps observations runtime-owned and outside this model-facing contract.
 */
const DecisionFieldsSchema = z
  .object({
    status: z
      .enum(['completed', 'needs_revision', 'blocked', 'failed'])
      .describe(
        'Terminal decision for the current node: completed when every criterion is satisfied; needs_revision when current-node evidence invalidates a planning assumption; blocked when completion is concretely impossible after reasonable alternatives; failed for an invalid result or terminal execution failure.',
      ),
    criteria: z
      .array(CriterionSchema)
      .describe(
        "Exactly one evaluation for each item in the current node's doneWhen array, preserving source order; criteria[i].criterionIndex must equal i.",
      ),
    result: ResultSchema.nullable().describe(
      'Final current-node result: a result object only when status is completed; null for needs_revision, blocked, or failed.',
    ),
    revisionRequest: RevisionRequestSchema.nullable().describe(
      'Requested structural plan change: a request object only when status is needs_revision; null for completed, blocked, or failed.',
    ),
    reason: z
      .string()
      .trim()
      .min(1)
      .nullable()
      .describe(
        'Why the current node did not complete: null when status is completed; otherwise a non-empty reason specific to needs_revision, blocked, or failed.',
      ),
  })
  .strict();

type Decision = z.output<typeof DecisionFieldsSchema>;
type RefinementContext = z.RefinementCtx;

/** Strict semantic decision contract authored by the executing model. */
export const NodeDecisionSchema = DecisionFieldsSchema.superRefine(
  (decision, context) => validateStatus(undefined, decision, context),
);

export type NodeDecision = z.output<typeof NodeDecisionSchema>;

/** Complete semantic outcome; observation objects remain on the producing node. */
export const NodeOutcomeSchema = DecisionFieldsSchema.superRefine(
  (outcome, context) => {
    validateStatus(undefined, outcome, context);
  },
);

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
export const createNodeOutcomeSchema = (node: Node, graph?: Graph) =>
  NodeOutcomeSchema.superRefine((outcome, context) => {
    validateCriteria(node, outcome, context);
    validateObservationIds(node, graph, outcome, context);
    if (outcome.status === 'needs_revision' && node.observations.length === 0) {
      context.addIssue({
        code: 'custom',
        path: ['revisionRequest'],
        message: 'A needs_revision outcome requires a local observation.',
      });
    }
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

const validateObservationIds = (
  node: Node,
  graph: Graph | undefined,
  outcome: Decision,
  context: RefinementContext,
): void => {
  const allowed =
    graph === undefined
      ? new Set(node.observations.map(({ id }) => id))
      : authorizedObservationIds(node, graph);

  outcome.criteria.forEach((criterion, criterionPosition) => {
    criterion.observationIds.forEach((observationId, idPosition) => {
      if (allowed.has(observationId)) return;

      context.addIssue({
        code: 'custom',
        path: ['criteria', criterionPosition, 'observationIds', idPosition],
        message: `Observation ID ${observationId} was not presented to node ${node.id}.`,
      });
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
