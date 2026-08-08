import { z } from 'zod';

import { HARNESS_COMMANDS } from '../core/commands.js';
import { artifactHash } from '../core/index.js';
import { PRIMARY_MODEL } from '../config/index.js';
import {
  CaseV1,
  ModelConfig,
  ScoreRowV1,
  type Case,
  type ScoreRow,
} from '../schemas/index.js';
import {
  PILOT_CASES,
  validateCases,
  validateFamilyIsolation,
  type CalibrationObservation,
} from '../study/index.js';
import type { CliInvocation } from './args.js';
import { rejectUnknownFlags } from './args.js';
import { readJson } from './io.js';
import { input, outputOrValue } from './shared.js';

const observation = z
  .object({
    caseId: z.string().trim().min(1),
    repetition: z.union([z.literal(1), z.literal(2), z.literal(3)]),
    lunaSuccess: z.union([z.literal(0), z.literal(1)]),
    candidateSuccess: z.union([z.literal(0), z.literal(1)]),
  })
  .strict();

const resultV1 = z
  .object({
    neutralCases: z.literal(60),
    repetitions: z.literal(3),
    difference: z.number().finite().min(-1).max(1),
    confidence95: z
      .object({
        lower: z.number().finite().min(-1).max(1),
        upper: z.number().finite().min(-1).max(1),
      })
      .strict(),
    approved: z.boolean(),
  })
  .strict();

const inputV1 = z
  .object({
    studyId: z.string().trim().min(1),
    candidate: ModelConfig.extend({ effort: z.literal('medium') }),
    seed: z.string().trim().min(1),
    cases: z.array(CaseV1),
    lunaRows: z.array(ScoreRowV1),
    candidateRows: z.array(ScoreRowV1),
  })
  .strict();

const artifactV1 = inputV1
  .extend({
    schemaVersion: z.literal(1),
    observations: z.array(observation),
    result: resultV1,
    artifactHash: z.string().regex(/^sha256:[a-f0-9]{64}$/u),
  })
  .strict();

const sameModel = (
  row: ScoreRow,
  model: z.infer<typeof ModelConfig>,
): boolean =>
  row.provider === model.provider &&
  row.model === model.model &&
  row.effort === model.effort;

const validateRows = (
  rows: readonly ScoreRow[],
  studyId: string,
  cases: readonly Case[],
  model: z.infer<typeof ModelConfig>,
): void => {
  const byCase = new Map(cases.map((entry) => [entry.id, entry]));
  const cells = new Set(
    rows.map(({ caseId, repetition }) => `${caseId}\r${repetition}`),
  );
  const complete = cases.every(({ id }) =>
    [1, 2, 3].every((repetition) => cells.has(`${id}\r${repetition}`)),
  );
  if (
    rows.length !== 180 ||
    cells.size !== rows.length ||
    !complete ||
    new Set(rows.map(({ runId }) => runId)).size !== rows.length ||
    rows.some((row) => {
      const benchmarkCase = byCase.get(row.caseId);
      return (
        benchmarkCase === undefined ||
        row.studyId !== studyId ||
        row.phase !== 'calibration' ||
        row.conditionId !== 'M1' ||
        row.repetition < 1 ||
        row.repetition > 3 ||
        row.pairedBlock !== `${row.caseId}.rep.${row.repetition}` ||
        !sameModel(row, model) ||
        row.freezeHash !== null ||
        row.modelCallBudget !== null ||
        row.primaryEligible ||
        row.familyId !== benchmarkCase.familyId ||
        row.domain !== benchmarkCase.domain ||
        row.compositionClass !== benchmarkCase.compositionClass ||
        row.adaptive !== benchmarkCase.adaptive
      );
    })
  ) {
    throw new TypeError(
      'calibration scores are not the complete paired M1 design',
    );
  }
};

const observations = (
  cases: readonly Case[],
  lunaRows: readonly ScoreRow[],
  candidateRows: readonly ScoreRow[],
): readonly CalibrationObservation[] => {
  const luna = new Map(
    lunaRows.map((row) => [`${row.caseId}\r${row.repetition}`, row]),
  );
  const candidate = new Map(
    candidateRows.map((row) => [`${row.caseId}\r${row.repetition}`, row]),
  );
  return [...cases]
    .sort((left, right) =>
      left.id < right.id ? -1 : left.id > right.id ? 1 : 0,
    )
    .flatMap(({ id }) =>
      ([1, 2, 3] as const).map((repetition) => {
        const key = `${id}\r${repetition}`;
        const lunaRow = luna.get(key);
        const candidateRow = candidate.get(key);
        if (
          lunaRow === undefined ||
          candidateRow === undefined ||
          lunaRow.pairedBlock !== candidateRow.pairedBlock
        ) {
          throw new TypeError('calibration model families are not paired');
        }
        return {
          caseId: id,
          repetition,
          lunaSuccess: lunaRow.success,
          candidateSuccess: candidateRow.success,
        };
      }),
    );
};

const materialize = (
  value: z.infer<typeof inputV1>,
): z.infer<typeof artifactV1> => {
  const caseIssues = validateCases(value.cases);
  const isolation = validateFamilyIsolation(PILOT_CASES, value.cases);
  if (
    value.cases.some(({ phase }) => phase !== 'calibration') ||
    caseIssues.length > 0 ||
    isolation.length > 0
  ) {
    throw new TypeError('calibration requires 60 independent valid cases');
  }
  validateRows(value.lunaRows, value.studyId, value.cases, PRIMARY_MODEL);
  validateRows(
    value.candidateRows,
    value.studyId,
    value.cases,
    value.candidate,
  );
  const paired = observations(value.cases, value.lunaRows, value.candidateRows);
  const result = HARNESS_COMMANDS.calibrateModels(paired, value.seed);
  const body = {
    schemaVersion: 1 as const,
    studyId: value.studyId,
    candidate: value.candidate,
    seed: value.seed,
    cases: value.cases,
    lunaRows: value.lunaRows,
    candidateRows: value.candidateRows,
    observations: paired,
    result,
  };
  return artifactV1.parse({ ...body, artifactHash: artifactHash(body) });
};

export const verifiedCalibration = async (
  path: string,
): Promise<z.infer<typeof artifactV1>> => {
  const value = artifactV1.parse(await readJson(path));
  const { artifactHash: declaredHash, ...body } = value;
  if (artifactHash(body) !== declaredHash) {
    throw new Error('calibration artifact hash mismatch');
  }
  const recomputed = materialize({
    studyId: value.studyId,
    candidate: value.candidate,
    seed: value.seed,
    cases: value.cases,
    lunaRows: value.lunaRows,
    candidateRows: value.candidateRows,
  });
  if (artifactHash(recomputed) !== artifactHash(value)) {
    throw new Error('calibration result is not reproducible');
  }
  return value;
};

export const calibrate = async (
  invocation: CliInvocation,
): Promise<unknown> => {
  rejectUnknownFlags(invocation, ['input', 'output']);
  return outputOrValue(
    invocation,
    materialize(inputV1.parse(await input(invocation))),
  );
};
