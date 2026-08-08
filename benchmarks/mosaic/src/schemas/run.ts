import { z } from 'zod';

import {
  Hash,
  Id,
  IsoDateTime,
  JsonValue,
  ModelConfig,
  NonNegativeInteger,
  PositiveInteger,
  SchemaVersion,
  StudyPhase,
  Usage,
} from './common.js';

export const RunSpecV1 = z
  .object({
    schemaVersion: SchemaVersion,
    id: Id,
    studyId: Id,
    phase: StudyPhase,
    caseId: Id,
    conditionId: Id,
    repetition: PositiveInteger,
    seed: z.number().int().safe(),
    pairedBlock: Id,
    order: NonNegativeInteger,
    model: ModelConfig,
    capture: z.enum(['structure', 'io']),
    freezeHash: Hash.nullable(),
    oracleParentRunId: Id.optional(),
    modelCallBudget: PositiveInteger.optional(),
  })
  .strict();

export const InfrastructureFailureV1 = z
  .object({
    stage: z.enum([
      'prepare',
      'model',
      'tool',
      'observer',
      'store',
      'score',
      'unknown',
    ]),
    code: Id,
    beforeFirstModelCall: z.boolean(),
  })
  .strict();

export const TraceReferenceV1 = z
  .object({
    rootHash: Hash,
    derivedHash: Hash,
    eventCount: NonNegativeInteger,
    relativePath: z.string().trim().min(1),
  })
  .strict();

/** Immutable record of one scheduled attempt, including infrastructure outcomes. */
export const ExecutionRecordV1 = z
  .object({
    schemaVersion: SchemaVersion,
    attempt: PositiveInteger,
    run: RunSpecV1,
    status: z.enum(['succeeded', 'failed', 'infrastructure']),
    startedAt: IsoDateTime,
    finishedAt: IsoDateTime,
    durationMs: NonNegativeInteger,
    firstModelCallStarted: z.boolean(),
    trace: TraceReferenceV1,
    outcome: JsonValue.nullable(),
    worldHash: Hash,
    usage: Usage,
    infrastructureFailure: InfrastructureFailureV1.nullable(),
  })
  .strict();

export type RunSpec = z.infer<typeof RunSpecV1>;
export type ExecutionRecord = z.infer<typeof ExecutionRecordV1>;
