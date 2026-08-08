import type {
  Case,
  ExecutionRecord,
  FreezeManifest,
  Review,
  ReviewAssignment,
} from '../schemas/index.js';

export interface DeterministicAssertion {
  readonly id: string;
  readonly passed: boolean;
  readonly required?: boolean;
}

export interface RetrievalScoreInput {
  readonly goalId: string;
  readonly ranked: readonly string[];
  readonly relevance?: Readonly<Record<string, number>>;
  readonly k: number;
}

export interface BundleScoreInput {
  readonly selected: readonly string[];
  readonly requiredBehaviors: readonly string[];
  readonly behaviorsBySkill: Readonly<Record<string, readonly string[]>>;
}

export interface MenuScoreInput {
  readonly offered: readonly string[];
  readonly required: readonly string[];
}

export interface ScoredObservation {
  readonly tool: string;
  readonly output: unknown;
}

export interface ObservationScoreInput {
  readonly actual: readonly ScoredObservation[];
  readonly expected: readonly ScoredObservation[];
}

export interface ScoreEvidence {
  readonly assertions: readonly DeterministicAssertion[];
  readonly retrievals?: readonly RetrievalScoreInput[];
  readonly selectedSkills?: readonly string[];
  readonly actualMenu?: readonly string[];
  readonly expectedMenu?: readonly string[];
  readonly observations?: ObservationScoreInput;
  readonly failureCode?: string;
}

export type ScoreEligibilityPolicy =
  | {
      readonly family: 'primary' | 'replication';
      readonly freeze: FreezeManifest;
    }
  | {
      readonly family: 'sensitivity';
      readonly freeze: FreezeManifest;
    }
  | {
      readonly family: 'exploratory';
    };

/** Versioned inputs whose output is validated by ScoreRowV1. */
export interface ScoreInput {
  readonly record: ExecutionRecord;
  readonly case: Case;
  readonly evidence: ScoreEvidence;
  readonly eligibility: ScoreEligibilityPolicy;
}

export interface ReviewArtifact {
  readonly hash: string;
}

export interface ReviewCase {
  readonly caseId: string;
  readonly cell: string;
  readonly repetitions: readonly number[];
  readonly artifacts: Readonly<Record<string, ReviewArtifact>>;
}

export interface ReviewPrepareInput {
  readonly studyId: string;
  readonly cases: readonly ReviewCase[];
  readonly conditions: readonly [string, string] | readonly string[];
  readonly seed: string;
  readonly rounds: readonly [string, string];
}

export interface ReviewKeyEntry {
  readonly assignmentId: string;
  readonly subjectId: string;
  readonly cell: string;
  readonly conditionId: string;
}

export interface PreparedReview {
  readonly schemaVersion: 1;
  readonly assignments: readonly ReviewAssignment[];
  readonly key: readonly ReviewKeyEntry[];
}

export interface ReviewStatus {
  readonly schemaVersion: 1;
  readonly assigned: number;
  readonly completedPassOne: number;
  readonly completedPassTwo: number;
  readonly stability: number | null;
  readonly judgeExtrapolationAllowed: boolean;
}

export interface PowerEstimate {
  readonly cases: number;
  readonly power: number;
}

export interface HypothesisPValue {
  readonly id: string;
  readonly p: number;
}

export interface AdjustedPValue extends HypothesisPValue {
  readonly adjusted: number;
}

export type ReviewInput = Review;
