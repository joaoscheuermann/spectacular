import type {
  AdjustedPValue,
  HypothesisPValue,
  PowerEstimate,
} from './types.js';
import type { ScoreRow } from '../schemas/index.js';

const validProbability = (value: number): boolean =>
  Number.isFinite(value) && value >= 0 && value <= 1;

/** Applies Holm's step-down family-wise error correction in input order. */
export const holmAdjust = (
  hypotheses: readonly HypothesisPValue[],
): readonly AdjustedPValue[] => {
  if (hypotheses.some(({ p }) => !validProbability(p))) {
    throw new TypeError('p-values must be finite numbers between zero and one');
  }
  const ranked = hypotheses
    .map((hypothesis, index) => ({ ...hypothesis, index }))
    .sort((left, right) => left.p - right.p || left.index - right.index);
  let previous = 0;
  const adjusted = ranked.map((hypothesis, index) => {
    const candidate = Math.min(1, hypothesis.p * (ranked.length - index));
    previous = Math.max(previous, candidate);
    return { ...hypothesis, adjusted: previous };
  });

  return adjusted
    .sort((left, right) => left.index - right.index)
    .map(({ index: _index, ...hypothesis }) => hypothesis);
};

/** Selects the first simulated design with at least 80% power. */
export const selectPoweredSampleSize = (
  estimates: readonly PowerEstimate[],
): number => {
  const invalid = estimates.some(
    ({ cases, power }) =>
      !Number.isSafeInteger(cases) ||
      cases <= 0 ||
      cases % 24 !== 0 ||
      !validProbability(power),
  );
  if (invalid)
    throw new TypeError('power estimates require valid multiples of 24');

  const selected = [...estimates]
    .sort((left, right) => left.cases - right.cases)
    .find(({ power }) => power >= 0.8);
  if (!selected) throw new Error('no simulated sample size reached 80% power');
  return selected.cases;
};

/** Applies the frozen minimum and 120-case balancing rule. */
export const finalSampleSize = (poweredCases: number): number => {
  if (!Number.isSafeInteger(poweredCases) || poweredCases <= 0) {
    throw new TypeError('powered sample size must be a positive safe integer');
  }
  const minimum = Math.max(240, poweredCases);
  return Math.ceil(minimum / 120) * 120;
};

/** Freezes the nearest-rank p95 model-call cap from every baseline pilot row. */
export const modelCallBudgetAtP95 = (
  rows: readonly ScoreRow[],
  baselineCondition: string,
): number => {
  const calls = rows
    .filter(
      ({ phase, conditionId }) =>
        phase === 'pilot' && conditionId === baselineCondition,
    )
    .map(({ modelCalls }) => modelCalls)
    .sort((left, right) => left - right);
  if (calls.length === 0) {
    throw new TypeError('budget sensitivity requires baseline pilot rows');
  }
  const budget = calls[Math.ceil(calls.length * 0.95) - 1] as number;
  if (budget === 0) {
    throw new Error('baseline pilot produced no positive model-call budget');
  }
  return budget;
};
