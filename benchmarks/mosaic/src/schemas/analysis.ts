import { z } from 'zod';

import {
  CompositionClass,
  Hash,
  Id,
  NonNegativeInteger,
  PositiveInteger,
  Probability,
  SchemaVersion,
} from './common.js';

export const IntervalV1 = z
  .object({
    lower: z.number().finite(),
    upper: z.number().finite(),
  })
  .strict();

export const ContrastV1 = z
  .object({
    id: Id,
    estimate: z.number().finite(),
    oddsRatio: z.number().finite().positive(),
    oddsRatioConfidence95: IntervalV1,
    absoluteDifference: z.number().finite().min(-1).max(1),
    absoluteDifferenceConfidence95: IntervalV1,
    pValue: Probability,
    holmPValue: Probability,
  })
  .strict();

export const PowerResultV1 = z
  .object({
    simulations: z.literal(10_000),
    seed: z.string().trim().min(1),
    numericSeed: z.number().int().safe(),
    configHash: Hash,
    baselineProbability: Probability,
    randomInterceptSd: z.number().finite().nonnegative(),
    adaptiveClasses: z.array(CompositionClass).min(1),
    nPower: PositiveInteger,
    nFinal: PositiveInteger,
    power: Probability,
    minimumEffect: z.literal(0.1),
  })
  .strict();

export const AnalysisResultV1 = z
  .object({
    schemaVersion: SchemaVersion,
    studyId: Id,
    family: z.enum(['primary', 'secondary', 'replication', 'sensitivity']),
    datasetHash: Hash,
    freezeHash: Hash,
    implementationHash: Hash,
    formula: z.literal(
      'success ~ condition * composition_class + (1 | case_id)',
    ),
    estimator: z.enum([
      'glmer-bobyqa',
      'glmer-nloptwrap',
      'glm-hc2-cluster',
      'case-bootstrap',
      'not-estimable',
    ]),
    estimable: z.boolean(),
    validFitRate: Probability,
    observations: NonNegativeInteger,
    cases: NonNegativeInteger,
    contrasts: z.array(ContrastV1),
    power: PowerResultV1.nullable(),
    warnings: z.array(Id),
    resultHash: Hash,
  })
  .strict();

export type AnalysisResult = z.infer<typeof AnalysisResultV1>;
