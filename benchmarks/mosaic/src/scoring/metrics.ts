import type {
  BundleScoreInput,
  MenuScoreInput,
  ObservationScoreInput,
} from './types.js';

const unique = (values: readonly string[]): readonly string[] => [
  ...new Set(values),
];

const fraction = (numerator: number, denominator: number): number =>
  denominator === 0 ? 1 : numerator / denominator;

const canonical = (value: unknown): string => {
  if (Array.isArray(value)) {
    return `[${value.map(canonical).join(',')}]`;
  }
  if (value !== null && typeof value === 'object') {
    const record = value as Readonly<Record<string, unknown>>;
    return `{${Object.keys(record)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonical(record[key])}`)
      .join(',')}}`;
  }
  return JSON.stringify(value);
};

const relevanceValues = (
  relevance: Readonly<Record<string, number>>,
): readonly number[] =>
  Object.values(relevance).filter(
    (value) => Number.isFinite(value) && value > 0,
  );

/** Returns the proportion of relevant items retrieved in the first K ranks. */
export const recallAtK = (
  ranked: readonly string[],
  relevance: Readonly<Record<string, number>>,
  k: number,
): number => {
  const relevant = new Set(
    Object.entries(relevance)
      .filter(([, value]) => Number.isFinite(value) && value > 0)
      .map(([id]) => id),
  );
  const hits = new Set(
    ranked.slice(0, Math.max(0, k)).filter((id) => relevant.has(id)),
  );
  return fraction(hits.size, relevant.size);
};

const discountedGain = (relevances: readonly number[]): number =>
  relevances.reduce(
    (total, relevance, index) =>
      total + (2 ** relevance - 1) / Math.log2(index + 2),
    0,
  );

/** Returns normalized discounted cumulative gain with graded relevance. */
export const ndcgAtK = (
  ranked: readonly string[],
  relevance: Readonly<Record<string, number>>,
  k: number,
): number => {
  const limit = Math.max(0, k);
  const actual = unique(ranked)
    .slice(0, limit)
    .map((id) => Math.max(0, relevance[id] ?? 0));
  const ideal = [...relevanceValues(relevance)]
    .sort((left, right) => right - left)
    .slice(0, limit);
  const idealGain = discountedGain(ideal);
  return idealGain === 0 ? 1 : discountedGain(actual) / idealGain;
};

export const scoreBundle = (
  input: BundleScoreInput,
): { readonly coverage: number; readonly redundancy: number } => {
  const required = new Set(unique(input.requiredBehaviors));
  const selected = unique(input.selected);
  const covered = new Set<string>();
  const redundant = selected.filter((skill) => {
    const before = covered.size;
    for (const behavior of input.behaviorsBySkill[skill] ?? []) {
      if (required.has(behavior)) covered.add(behavior);
    }
    return covered.size === before;
  }).length;

  return {
    coverage: fraction(covered.size, required.size),
    redundancy: selected.length === 0 ? 0 : redundant / selected.length,
  };
};

export const scoreMenu = (
  input: MenuScoreInput,
): { readonly coverage: number; readonly extraneousRate: number } => {
  const offered = new Set(input.offered);
  const required = new Set(input.required);
  const covered = [...required].filter((tool) => offered.has(tool)).length;
  const extraneous = [...offered].filter((tool) => !required.has(tool)).length;
  return {
    coverage: fraction(covered, required.size),
    extraneousRate: offered.size === 0 ? 0 : extraneous / offered.size,
  };
};

export const scoreObservations = (
  input: ObservationScoreInput,
): { readonly accuracy: number; readonly exact: boolean } => {
  const expected = input.expected.map(canonical);
  const actual = input.actual.map(canonical);
  const matches = expected.filter(
    (value, index) => value === actual[index],
  ).length;
  const denominator = Math.max(expected.length, actual.length);
  return {
    accuracy: fraction(matches, denominator),
    exact:
      expected.length === actual.length &&
      expected.every((value, index) => value === actual[index]),
  };
};
