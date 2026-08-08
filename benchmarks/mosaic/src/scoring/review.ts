import { createHash } from 'node:crypto';

import type { Review, ReviewAssignment } from '../schemas/index.js';
import {
  ReviewAssignmentV1,
  ReviewStatusV1,
  ReviewV1,
} from '../schemas/index.js';
import type {
  PreparedReview,
  ReviewCase,
  ReviewKeyEntry,
  ReviewPrepareInput,
  ReviewStatus,
} from './types.js';

const DAY_MS = 24 * 60 * 60 * 1000;

const digest = (...parts: readonly string[]): string =>
  createHash('sha256').update(parts.join('\0')).digest('hex');

const parseInstant = (value: string): number => {
  const instant = Date.parse(value);
  if (!Number.isFinite(instant) || new Date(instant).toISOString() !== value) {
    throw new TypeError('review dates must be canonical ISO-8601 instants');
  }
  return instant;
};

const validateRounds = (rounds: readonly [string, string]): void => {
  if (parseInstant(rounds[1]) - parseInstant(rounds[0]) < 14 * DAY_MS) {
    throw new TypeError('review rounds must be separated by at least 14 days');
  }
};

const groupCases = (
  cases: readonly ReviewCase[],
): ReadonlyMap<string, readonly ReviewCase[]> => {
  const groups = new Map<string, ReviewCase[]>();
  cases.forEach((item) => {
    groups.set(item.cell, [...(groups.get(item.cell) ?? []), item]);
  });
  return groups;
};

const chooseCases = (
  cases: readonly ReviewCase[],
  cell: string,
  seed: string,
): readonly ReviewCase[] => {
  if (cases.length < 5 || cases.length % 5 !== 0) {
    throw new TypeError(
      `review cell ${cell} must contain a positive multiple of five cases`,
    );
  }
  return [...cases]
    .sort((left, right) =>
      digest(seed, cell, left.caseId).localeCompare(
        digest(seed, cell, right.caseId),
      ),
    )
    .slice(0, cases.length / 5);
};

const chooseRepetition = (item: ReviewCase, seed: string): number => {
  const repetitions = [...new Set(item.repetitions)].sort(
    (left, right) => left - right,
  );
  if (
    repetitions.length === 0 ||
    repetitions.some((value) => !Number.isSafeInteger(value) || value < 1)
  ) {
    throw new TypeError(
      `review case ${item.caseId} has no positive repetition`,
    );
  }
  const offset = Number.parseInt(digest(seed, item.caseId).slice(0, 8), 16);
  return repetitions[offset % repetitions.length] as number;
};

const makeAssignment = (input: {
  readonly studyId: string;
  readonly item: ReviewCase;
  readonly conditionId: string;
  readonly repetition: number;
  readonly pass: 1 | 2;
  readonly notBefore: string;
  readonly seed: string;
}): { readonly assignment: ReviewAssignment; readonly key: ReviewKeyEntry } => {
  const conditionCode = digest(
    input.seed,
    'condition',
    input.conditionId,
  ).slice(0, 32);
  const subjectId = digest(
    input.seed,
    input.item.caseId,
    input.conditionId,
    String(input.repetition),
  );
  const id = digest(input.seed, subjectId, String(input.pass));
  const artifact = input.item.artifacts[input.conditionId];
  if (!artifact)
    throw new TypeError(
      `review case ${input.item.caseId} is missing an artifact`,
    );

  return {
    assignment: ReviewAssignmentV1.parse({
      schemaVersion: 1,
      id,
      studyId: input.studyId,
      caseId: input.item.caseId,
      conditionCode,
      runCode: digest(input.seed, 'run', subjectId).slice(0, 32),
      repetition: input.repetition,
      pass: input.pass,
      notBefore: input.notBefore,
      artifactHash: artifact.hash,
      artifactPath: `artifacts/review/${artifact.hash.slice('sha256:'.length)}`,
    }),
    key: {
      assignmentId: id,
      subjectId,
      cell: input.item.cell,
      conditionId: input.conditionId,
    },
  };
};

const entriesForCase = (
  input: ReviewPrepareInput,
  item: ReviewCase,
): readonly {
  readonly assignment: ReviewAssignment;
  readonly key: ReviewKeyEntry;
}[] => {
  const repetition = chooseRepetition(item, input.seed);
  return input.conditions.flatMap((conditionId) =>
    input.rounds.map((notBefore, index) =>
      makeAssignment({
        studyId: input.studyId,
        item,
        conditionId,
        repetition,
        pass: (index + 1) as 1 | 2,
        notBefore,
        seed: input.seed,
      }),
    ),
  );
};

/** Creates blinded, paired assignments for exactly 20% of each review cell. */
export const prepareReview = (input: ReviewPrepareInput): PreparedReview => {
  const conditions = new Set(input.conditions);
  const baseline = input.conditions.find((condition) => condition !== 'M1');
  if (
    conditions.size !== 2 ||
    !conditions.has('M1') ||
    baseline === undefined ||
    !/^B[0-3]$/.test(baseline)
  ) {
    throw new TypeError(
      'review preparation requires M1 and one frozen baseline',
    );
  }
  validateRounds(input.rounds);
  if (
    new Set(input.cases.map(({ caseId }) => caseId)).size !== input.cases.length
  ) {
    throw new TypeError('review case IDs must be unique');
  }

  const entries = [...groupCases(input.cases).entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .flatMap(([cell, cases]) => chooseCases(cases, cell, input.seed))
    .flatMap((item) => entriesForCase(input, item))
    .sort((left, right) =>
      left.assignment.id.localeCompare(right.assignment.id),
    );

  return {
    schemaVersion: 1,
    assignments: entries.map(({ assignment }) => assignment),
    key: entries.map(({ key }) => key),
  };
};

/** Computes Cohen's kappa for paired binary test-retest decisions. */
export const stability = (
  pairs: readonly (readonly [boolean, boolean])[],
): number | null => {
  if (pairs.length === 0) return null;
  const observed =
    pairs.filter(([first, second]) => first === second).length / pairs.length;
  const firstPositive = pairs.filter(([first]) => first).length / pairs.length;
  const secondPositive =
    pairs.filter(([, second]) => second).length / pairs.length;
  const expected =
    firstPositive * secondPositive + (1 - firstPositive) * (1 - secondPositive);
  if (expected === 1) return null;
  return (observed - expected) / (1 - expected);
};

const validateReviews = (
  prepared: PreparedReview,
  reviews: readonly Review[],
): readonly Review[] => {
  const assignments = new Map(
    prepared.assignments.map((assignment) => [assignment.id, assignment]),
  );
  const parsed = reviews.map((review) => ReviewV1.parse(review));
  if (
    new Set(parsed.map(({ assignmentId }) => assignmentId)).size !==
    parsed.length
  ) {
    throw new TypeError('each assignment accepts only one review');
  }
  if (new Set(parsed.map(({ reviewerCode }) => reviewerCode)).size > 1) {
    throw new TypeError(
      'the semantic review protocol requires one solo reviewer',
    );
  }
  parsed.forEach((review) => {
    const assignment = assignments.get(review.assignmentId);
    if (!assignment)
      throw new TypeError('review references an unknown assignment');
    if (
      assignment.notBefore !== null &&
      parseInstant(review.submittedAt) < parseInstant(assignment.notBefore)
    ) {
      throw new TypeError(
        'review was submitted before its pass became available',
      );
    }
  });
  return parsed;
};

const pairedDecisions = (
  assignments: readonly ReviewAssignment[],
  reviews: readonly Review[],
): readonly (readonly [boolean, boolean])[] => {
  const reviewById = new Map(
    reviews.map((review) => [review.assignmentId, review]),
  );
  const bySubject = new Map<string, ReviewAssignment[]>();
  assignments.forEach((assignment) => {
    const subject = `${assignment.caseId}\0${assignment.conditionCode}\0${assignment.repetition}`;
    bySubject.set(subject, [...(bySubject.get(subject) ?? []), assignment]);
  });

  return [...bySubject.values()].flatMap((entries) => {
    const first = entries.find(({ pass }) => pass === 1);
    const second = entries.find(({ pass }) => pass === 2);
    const firstReview = first ? reviewById.get(first.id) : undefined;
    const secondReview = second ? reviewById.get(second.id) : undefined;
    if (!firstReview || !secondReview) return [];
    if (
      parseInstant(secondReview.submittedAt) -
        parseInstant(firstReview.submittedAt) <
      14 * DAY_MS
    ) {
      throw new TypeError(
        'paired reviews must be submitted at least 14 days apart',
      );
    }
    return [[firstReview.acceptable, secondReview.acceptable]] as const;
  });
};

/** Adds one schema-valid review after enforcing assignment and blinding rules. */
export const ingestReview = (
  prepared: PreparedReview,
  existing: readonly Review[],
  incoming: Review,
): readonly Review[] => validateReviews(prepared, [...existing, incoming]);

/** Summarizes both passes and the frozen 0.80 auxiliary-judge gate. */
export const reviewStatus = (
  prepared: PreparedReview,
  reviews: readonly Review[],
): ReviewStatus => {
  const valid = validateReviews(prepared, reviews);
  const byId = new Set(valid.map(({ assignmentId }) => assignmentId));
  const humanStability = stability(
    pairedDecisions(prepared.assignments, valid),
  );
  return ReviewStatusV1.parse({
    schemaVersion: 1,
    assigned: prepared.assignments.length,
    completedPassOne: prepared.assignments.filter(
      ({ id, pass }) => pass === 1 && byId.has(id),
    ).length,
    completedPassTwo: prepared.assignments.filter(
      ({ id, pass }) => pass === 2 && byId.has(id),
    ).length,
    stability: humanStability,
    judgeExtrapolationAllowed:
      valid.length === prepared.assignments.length &&
      humanStability !== null &&
      humanStability >= 0.8,
  });
};
