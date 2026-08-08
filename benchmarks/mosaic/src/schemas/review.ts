import { z } from 'zod';

import {
  Hash,
  Id,
  IsoDateTime,
  NonNegativeInteger,
  Probability,
  SchemaVersion,
} from './common.js';

export const ReviewAssignmentV1 = z
  .object({
    schemaVersion: SchemaVersion,
    id: Id,
    studyId: Id,
    caseId: Id,
    conditionCode: Id,
    runCode: Id,
    repetition: z.number().int().positive().safe(),
    pass: z.union([z.literal(1), z.literal(2)]),
    notBefore: IsoDateTime.nullable(),
    artifactHash: Hash,
    artifactPath: z.string().trim().min(1),
  })
  .strict();

export const CriterionReviewV1 = z
  .object({
    criterionId: Id,
    satisfied: z.boolean(),
  })
  .strict();

export const AuxiliaryJudgeV1 = z
  .object({
    model: z.literal('openai/gpt-5.6-sol'),
    verdict: z.boolean(),
    confidence: Probability,
    promptHash: Hash,
  })
  .strict();

export const ReviewV1 = z
  .object({
    schemaVersion: SchemaVersion,
    assignmentId: Id,
    reviewerCode: Id,
    submittedAt: IsoDateTime,
    acceptable: z.boolean(),
    criteria: z.array(CriterionReviewV1).min(1),
    confidence: z.number().int().min(1).max(5),
    notes: z.string(),
    auxiliaryJudge: AuxiliaryJudgeV1.nullable(),
  })
  .strict();

export const ReviewStatusV1 = z
  .object({
    schemaVersion: SchemaVersion,
    assigned: NonNegativeInteger,
    completedPassOne: NonNegativeInteger,
    completedPassTwo: NonNegativeInteger,
    stability: z.number().finite().min(-1).max(1).nullable(),
    judgeExtrapolationAllowed: z.boolean(),
  })
  .strict();

export type ReviewAssignment = z.infer<typeof ReviewAssignmentV1>;
export type Review = z.infer<typeof ReviewV1>;
