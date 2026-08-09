import { z } from 'zod';

import {
  Hash,
  Id,
  IsoDateTime,
  ModelConfig,
  PositiveInteger,
  SchemaVersion,
} from './common.js';

const ArtifactHashesV1 = z
  .object({
    protocol: Hash,
    schemas: Hash,
    catalog: Hash,
    tools: Hash,
    pilotCases: Hash,
    pilotScores: Hash,
    confirmatoryCases: Hash,
    conditions: Hash,
    prompts: Hash,
    prices: Hash,
    seeds: Hash,
    calibration: Hash,
    calibrationAudit: Hash,
    confirmatoryAudit: Hash,
    costApproval: Hash,
    powerConfig: Hash,
    powerApproval: Hash,
    powerResult: Hash,
    retrievalIndex: Hash,
    analysis: Hash,
    renvLock: Hash,
  })
  .strict();

const ReplicationCalibrationV1 = z
  .object({
    candidate: ModelConfig.extend({ effort: z.literal('medium') }),
    neutralCases: z.literal(60),
    repetitions: z.literal(3),
    difference: z.number().finite().min(-1).max(1),
    confidence95: z
      .object({
        lower: z.number().finite().min(-1).max(1),
        upper: z.number().finite().min(-1).max(1),
      })
      .strict(),
    approved: z.literal(true),
  })
  .strict()
  .superRefine((value, context) => {
    if (
      value.candidate.provider.toLocaleLowerCase('en-US') === 'openai' ||
      value.candidate.model.toLocaleLowerCase('en-US').startsWith('openai/')
    ) {
      context.addIssue({
        code: 'custom',
        message: 'replication candidate must belong to a non-OpenAI family',
        path: ['candidate'],
      });
    }
    if (
      value.confidence95.lower > value.confidence95.upper ||
      value.difference < value.confidence95.lower ||
      value.difference > value.confidence95.upper
    ) {
      context.addIssue({
        code: 'custom',
        message: 'replication estimate must lie inside an ordered interval',
        path: ['confidence95'],
      });
    }
    if (value.confidence95.lower < -0.05 || value.confidence95.upper > 0.05) {
      context.addIssue({
        code: 'custom',
        message:
          'replication confidence interval must stay within [-0.05, 0.05]',
        path: ['confidence95'],
      });
    }
  });

/** Content-addressed manifest that closes all confirmatory study choices. */
export const FreezeManifestV1 = z
  .object({
    schemaVersion: SchemaVersion,
    studyId: Id,
    frozenAt: IsoDateTime,
    gitCommit: z.string().regex(/^[a-f0-9]{40,64}$/),
    cleanWorktree: z.literal(true),
    primaryModel: ModelConfig.extend({
      model: z.literal('openai/gpt-5.6-luna'),
      effort: z.literal('medium'),
    }),
    reranker: z.literal('voyageai/rerank-2.5-lite'),
    embedder: z
      .object({
        model: z.literal('voyageai/voyage-4-large'),
        dimensions: z.literal(2048),
      })
      .strict(),
    selectedBaseline: z.enum(['B0', 'B1', 'B2', 'B3']),
    repetitions: z.literal(5),
    alpha: z.literal(0.05),
    minimumEffect: z.literal(0.1),
    targetPower: z.literal(0.8),
    nPower: PositiveInteger,
    nFinal: PositiveInteger,
    /** Nearest-rank p95 baseline pilot call count, including failed runs. */
    modelCallBudgetP95: PositiveInteger,
    semanticReviewFraction: z.literal(0.2),
    artifactHashes: ArtifactHashesV1,
    replication: ReplicationCalibrationV1,
    containerDigest: z.string().regex(/^sha256:[a-f0-9]{64}$/),
    manifestHash: Hash,
  })
  .strict()
  .superRefine((value, context) => {
    if (value.nPower % 24 !== 0) {
      context.addIssue({
        code: 'custom',
        message: 'nPower must be a multiple of 24',
        path: ['nPower'],
      });
    }
    const expected = Math.ceil(Math.max(240, value.nPower) / 120) * 120;
    if (value.nFinal !== expected) {
      context.addIssue({
        code: 'custom',
        message:
          'nFinal must apply the frozen minimum and 120-case rounding rule',
        path: ['nFinal'],
      });
    }
  });

export type FreezeManifest = z.infer<typeof FreezeManifestV1>;
