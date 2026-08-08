import { createPrng } from '../core/prng.js';

export interface CalibrationObservation {
  readonly caseId: string;
  readonly repetition: 1 | 2 | 3;
  readonly lunaSuccess: 0 | 1;
  readonly candidateSuccess: 0 | 1;
}

export interface CalibrationResult {
  readonly neutralCases: 60;
  readonly repetitions: 3;
  readonly difference: number;
  readonly confidence95: { readonly lower: number; readonly upper: number };
  readonly approved: boolean;
}

const quantile = (sorted: readonly number[], probability: number): number => {
  const index = Math.min(
    sorted.length - 1,
    Math.floor(probability * sorted.length),
  );
  return sorted[index] as number;
};

/** Runs the frozen paired case bootstrap used by `calibrate-models`. */
export const calibrateModels = (
  observations: readonly CalibrationObservation[],
  seed: string,
  samples = 10_000,
): CalibrationResult => {
  const caseIds = [
    ...new Set(observations.map((entry) => entry.caseId)),
  ].sort();
  if (
    caseIds.length !== 60 ||
    observations.length !== 180 ||
    samples !== 10_000
  ) {
    throw new TypeError(
      'calibration requires 60 cases, three repetitions, and 10,000 bootstrap samples',
    );
  }
  if (
    observations.some(
      (entry) =>
        ![1, 2, 3].includes(entry.repetition) ||
        ![0, 1].includes(entry.lunaSuccess) ||
        ![0, 1].includes(entry.candidateSuccess),
    )
  ) {
    throw new TypeError(
      'calibration observations contain an invalid repetition or binary outcome',
    );
  }
  const byCase = caseIds.map((caseId) => {
    const rows = observations.filter((entry) => entry.caseId === caseId);
    if (
      rows.length !== 3 ||
      new Set(rows.map((entry) => entry.repetition)).size !== 3
    ) {
      throw new TypeError(
        `calibration case ${caseId} lacks three unique repetitions`,
      );
    }
    return (
      rows.reduce(
        (sum, entry) => sum + entry.candidateSuccess - entry.lunaSuccess,
        0,
      ) / 3
    );
  });
  const difference =
    byCase.reduce((sum, value) => sum + value, 0) / byCase.length;
  const rng = createPrng(seed);
  const bootstrap = Array.from({ length: samples }, () => {
    const selected = Array.from(
      { length: byCase.length },
      () => byCase[rng.integer(byCase.length)] as number,
    );
    return selected.reduce((sum, value) => sum + value, 0) / selected.length;
  }).sort((left, right) => left - right);
  const confidence95 = {
    lower: quantile(bootstrap, 0.025),
    upper: quantile(bootstrap, 0.975),
  };
  return {
    neutralCases: 60,
    repetitions: 3,
    difference,
    confidence95,
    approved: confidence95.lower >= -0.05 && confidence95.upper <= 0.05,
  };
};
