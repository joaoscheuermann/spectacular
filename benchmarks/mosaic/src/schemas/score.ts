import { z } from 'zod';

import {
  CompositionClass,
  Domain,
  Hash,
  Id,
  NonNegativeInteger,
  Probability,
  SchemaVersion,
  StudyPhase,
} from './common.js';

export const RetrievalMetricsV1 = z
  .object({
    recallAtK: Probability,
    ndcg: Probability,
    k: NonNegativeInteger,
  })
  .strict();

/** Flat, analysis-ready row derived deterministically from an execution record. */
export const ScoreRowV1 = z
  .object({
    schemaVersion: SchemaVersion,
    runId: Id,
    attempt: z.number().int().positive().safe(),
    studyId: Id,
    phase: StudyPhase,
    caseId: Id,
    familyId: Id,
    conditionId: Id,
    repetition: z.number().int().positive().safe(),
    pairedBlock: Id,
    provider: Id,
    model: z.string().trim().min(1),
    effort: z.enum(['none', 'minimal', 'low', 'medium', 'high', 'xhigh']),
    freezeHash: Hash.nullable(),
    traceRootHash: Hash,
    traceDerivedHash: Hash,
    evidenceHash: Hash,
    worldHash: Hash,
    modelCallBudget: z.number().int().positive().safe().nullable(),
    domain: Domain,
    compositionClass: CompositionClass,
    adaptive: z.boolean(),
    success: z.union([z.literal(0), z.literal(1)]),
    primaryEligible: z.boolean(),
    infrastructure: z.boolean(),
    failureCode: Id.nullable(),
    retrieval: RetrievalMetricsV1.nullable(),
    bundleExact: z.boolean().nullable(),
    menuExact: z.boolean().nullable(),
    observationExact: z.boolean().nullable(),
    inputTokens: NonNegativeInteger,
    outputTokens: NonNegativeInteger,
    modelCalls: NonNegativeInteger,
    toolCalls: NonNegativeInteger,
    costUsd: z.number().finite().nonnegative(),
    durationMs: NonNegativeInteger,
  })
  .strict();

export type ScoreRow = z.infer<typeof ScoreRowV1>;
