import type { Condition } from '../schemas/index.js';
import { ABLATIONS, B0, B1, B2, B3, M0, M1 } from './definitions.js';

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

export interface ControlParityIssue {
  readonly conditionId: string;
  readonly control: 'maxCandidates' | 'maxTurns' | 'structuredRepairRetries';
  readonly expected: number;
  readonly actual: number | null;
}

/** Proves undeclared retrieval, turn, and repair controls remain common. */
export const validatePrimaryControlParity =
  (): readonly ControlParityIssue[] => {
    const common = [B0, B1, B2, B3, M0, M1].flatMap((condition) =>
      (['maxTurns', 'structuredRepairRetries'] as const).flatMap((control) =>
        condition.factors[control] === M1.factors[control]
          ? []
          : [
              {
                conditionId: condition.id,
                control,
                expected: M1.factors[control]!,
                actual: condition.factors[control],
              },
            ],
      ),
    );
    const retrieval = [B1, B2, B3, M0, M1].flatMap((condition) =>
      condition.factors.maxCandidates === M1.factors.maxCandidates
        ? []
        : [
            {
              conditionId: condition.id,
              control: 'maxCandidates' as const,
              expected: M1.factors.maxCandidates!,
              actual: condition.factors.maxCandidates,
            },
          ],
    );
    return [...common, ...retrieval];
  };
