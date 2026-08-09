export * from './analysis.js';
export * from './case.js';
export * from './common.js';
export * from './condition.js';
export * from './freeze.js';
export * from './review.js';
export * from './run.js';
export * from './score.js';

import { AnalysisResultV1 } from './analysis.js';
import { CaseV1 } from './case.js';
import { ConditionV1 } from './condition.js';
import { FreezeManifestV1 } from './freeze.js';
import { ReviewAssignmentV1, ReviewV1 } from './review.js';
import { AttemptReservationV1, ExecutionRecordV1, RunSpecV1 } from './run.js';
import { ScoreRowV1 } from './score.js';

export const schemasV1 = {
  AnalysisResultV1,
  AttemptReservationV1,
  CaseV1,
  ConditionV1,
  ExecutionRecordV1,
  FreezeManifestV1,
  ReviewAssignmentV1,
  ReviewV1,
  RunSpecV1,
  ScoreRowV1,
} as const;
