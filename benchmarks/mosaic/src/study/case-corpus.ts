import { CaseV1, type Case } from '../schemas/index.js';
import { validateCalibrationCases, type CaseIssue } from './case-validation.js';

export interface CalibrationCaseReport {
  readonly schemaVersion: 1;
  readonly valid: boolean;
  readonly cases: readonly Case[];
  readonly issues: readonly CaseIssue[];
  readonly humanAuditRequired: readonly [
    'semantic-neutrality',
    'cross-phase-independence',
  ];
}

/** Parses every case independently so standalone validation reports all issues. */
export const validateCalibrationCorpus = (
  input: unknown,
  pilot: readonly Case[],
  confirmatory: readonly Case[] = [],
): CalibrationCaseReport => {
  if (!Array.isArray(input)) {
    return {
      schemaVersion: 1,
      valid: false,
      cases: [],
      issues: [
        {
          code: 'corpus_schema',
          caseId: '$',
          detail: 'calibration corpus must be a JSON array',
        },
      ],
      humanAuditRequired: ['semantic-neutrality', 'cross-phase-independence'],
    };
  }
  const parsed = input.map((value, index) => {
    const result = CaseV1.safeParse(value);
    const id =
      typeof value === 'object' &&
      value !== null &&
      !Array.isArray(value) &&
      typeof (value as Readonly<Record<string, unknown>>)['id'] === 'string'
        ? String((value as Readonly<Record<string, unknown>>)['id'])
        : `$[${index}]`;
    return result.success
      ? { cases: [result.data], issues: [] }
      : {
          cases: [],
          issues: result.error.issues.map((issue) => ({
            code: 'case_schema',
            caseId: id,
            detail: `${issue.path.join('.') || '$'}: ${issue.message}`,
          })),
        };
  });
  const cases = parsed.flatMap((result) => result.cases);
  const issues = [
    ...parsed.flatMap((result) => result.issues),
    ...validateCalibrationCases(cases, pilot, confirmatory),
  ];
  return {
    schemaVersion: 1,
    valid: issues.length === 0,
    cases,
    issues,
    humanAuditRequired: ['semantic-neutrality', 'cross-phase-independence'],
  };
};
