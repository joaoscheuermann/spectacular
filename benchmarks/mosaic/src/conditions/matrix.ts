import type { Condition } from '../schemas/index.js';
import { ABLATIONS, M1 } from './definitions.js';

export type FactorName = keyof Condition['factors'];

export interface MatrixIssue {
  readonly conditionId: string;
  readonly expected: readonly FactorName[];
  readonly actual: readonly FactorName[];
}

export const changedFactors = (
  left: Condition,
  right: Condition,
): readonly FactorName[] =>
  (Object.keys(left.factors) as readonly FactorName[]).filter(
    (key) => left.factors[key] !== right.factors[key],
  );

const EXPECTED: Readonly<Record<string, readonly FactorName[]>> = {
  A1: ['skillView'],
  A2: ['bundleOrder'],
  A3: ['baseTools'],
  A4: ['bundle'],
  A5: ['localizedRevision'],
};

/** Proves each named ablation changes exactly its declared M1 factor. */
export const validateAblationMatrix = (): readonly MatrixIssue[] =>
  ABLATIONS.flatMap((entry) => {
    const expected = EXPECTED[entry.id] ?? [];
    const actual = changedFactors(M1, entry);
    return actual.length === expected.length &&
      actual.every((value, index) => value === expected[index])
      ? []
      : [{ conditionId: entry.id, expected, actual }];
  });
