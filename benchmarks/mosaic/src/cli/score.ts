import { z } from 'zod';

import { artifactHash } from '../core/index.js';
import { createEventStore, createRecordStore } from '../runtime/index.js';
import {
  RunSpecV1,
  type FreezeManifest,
  type ScoreRow,
} from '../schemas/index.js';
import {
  scoreExecution,
  type ScoreEligibilityPolicy,
} from '../scoring/index.js';
import {
  PILOT_CASES,
  confirmatoryCasesHash,
  evaluateEvidence,
  resumeAction,
  seedLedgerHash,
  toScoreEvidence,
  validateConfirmatoryCases,
} from '../study/index.js';
import type { CliInvocation } from './args.js';
import { optionalFlag, rejectUnknownFlags, requiredFlag } from './args.js';
import { readJson, writeJsonExclusive, writeTextExclusive } from './io.js';
import { loadBenchmarkCases, loadFreezeManifest } from './run.js';

const eligibility = (
  family: 'exploratory' | 'primary' | 'replication' | 'sensitivity',
  freeze: FreezeManifest | undefined,
): ScoreEligibilityPolicy => {
  if (family === 'exploratory') return { family };
  if (freeze === undefined) {
    throw new TypeError(`${family} scoring requires --freeze`);
  }
  return { family, freeze };
};

export const validateFrozenScoreCorpus = (
  family: 'primary' | 'replication' | 'sensitivity',
  freeze: FreezeManifest,
  cases: Awaited<ReturnType<typeof loadBenchmarkCases>>,
  schedule: readonly z.infer<typeof RunSpecV1>[],
): void => {
  if (
    confirmatoryCasesHash(cases) !== freeze.artifactHashes.confirmatoryCases
  ) {
    throw new Error('score corpus differs from the frozen confirmatory cases');
  }
  const caseIssues = validateConfirmatoryCases(
    cases,
    PILOT_CASES,
    freeze.nFinal,
  );
  if (caseIssues.length > 0) {
    throw new TypeError(
      `frozen confirmatory corpus is invalid: ${caseIssues[0]?.code ?? 'unknown'}`,
    );
  }
  const expectedPhase =
    family === 'replication' ? 'replication' : 'confirmatory';
  const expectedConditions = new Set([freeze.selectedBaseline, 'M1']);
  const expectedModel =
    family === 'replication'
      ? freeze.replication.candidate
      : freeze.primaryModel;
  const scheduledCases = new Set(schedule.map(({ caseId }) => caseId));
  const scheduledCells = new Set(
    schedule.map(
      ({ caseId, conditionId, repetition }) =>
        `${caseId}\r${conditionId}\r${repetition}`,
    ),
  );
  const completeGrid = cases.every(({ id }) =>
    [...expectedConditions].every((conditionId) =>
      Array.from({ length: freeze.repetitions }, (_, index) => index + 1).every(
        (repetition) =>
          scheduledCells.has(`${id}\r${conditionId}\r${repetition}`),
      ),
    ),
  );
  const pairings = new Map<
    string,
    { readonly block: string; readonly seed: number }
  >();
  let pairingMismatch = false;
  for (const run of schedule) {
    const key = `${run.caseId}\r${run.repetition}`;
    const prior = pairings.get(key);
    if (prior === undefined) {
      pairings.set(key, { block: run.pairedBlock, seed: run.seed });
    } else if (prior.block !== run.pairedBlock || prior.seed !== run.seed) {
      pairingMismatch = true;
    }
  }
  if (
    schedule.length !== freeze.nFinal * freeze.repetitions * 2 ||
    scheduledCells.size !== schedule.length ||
    !completeGrid ||
    pairingMismatch ||
    scheduledCases.size !== freeze.nFinal ||
    new Set(schedule.map(({ pairedBlock }) => pairedBlock)).size !==
      freeze.nFinal * freeze.repetitions ||
    new Set(schedule.map(({ order }) => order)).size !== schedule.length ||
    Math.min(...schedule.map(({ order }) => order)) !== 0 ||
    Math.max(...schedule.map(({ order }) => order)) !== schedule.length - 1 ||
    cases.some(({ id }) => !scheduledCases.has(id)) ||
    schedule.some(
      ({
        studyId,
        phase,
        conditionId,
        model,
        capture,
        freezeHash,
        modelCallBudget,
        oracleParentRunId,
      }) =>
        studyId !== freeze.studyId ||
        phase !== expectedPhase ||
        !expectedConditions.has(conditionId) ||
        artifactHash(model) !== artifactHash(expectedModel) ||
        capture !== 'structure' ||
        freezeHash !== freeze.manifestHash ||
        oracleParentRunId !== undefined ||
        (family === 'sensitivity'
          ? modelCallBudget !== freeze.modelCallBudgetP95
          : modelCallBudget !== undefined),
    )
  ) {
    throw new TypeError(
      'score schedule is not the complete frozen paired study',
    );
  }
  if (seedLedgerHash(schedule) !== freeze.artifactHashes.seeds) {
    throw new Error(
      'score schedule differs from the frozen paired seed ledger',
    );
  }
};

const csvCell = (value: unknown): string => {
  const text =
    value === null || value === undefined
      ? ''
      : typeof value === 'object'
        ? JSON.stringify(value)
        : String(value);
  return `"${text.replaceAll('"', '""')}"`;
};

const columns = [
  'schemaVersion',
  'runId',
  'attempt',
  'studyId',
  'phase',
  'caseId',
  'familyId',
  'conditionId',
  'repetition',
  'pairedBlock',
  'provider',
  'model',
  'effort',
  'freezeHash',
  'traceRootHash',
  'traceDerivedHash',
  'evidenceHash',
  'worldHash',
  'modelCallBudget',
  'domain',
  'compositionClass',
  'adaptive',
  'success',
  'primaryEligible',
  'infrastructure',
  'failureCode',
  'retrieval',
  'bundleExact',
  'menuExact',
  'observationExact',
  'inputTokens',
  'outputTokens',
  'modelCalls',
  'toolCalls',
  'costUsd',
  'durationMs',
] as const satisfies readonly (keyof ScoreRow)[];

const csv = (rows: readonly ScoreRow[]): string =>
  [
    columns.join(','),
    ...rows.map((row) =>
      columns.map((column) => csvCell(row[column])).join(','),
    ),
  ].join('\n') + '\n';

export const score = async (invocation: CliInvocation): Promise<unknown> => {
  rejectUnknownFlags(invocation, [
    'schedule',
    'artifacts',
    'cases',
    'freeze',
    'family',
    'output',
    'csv',
  ]);
  const schedule = z
    .array(RunSpecV1)
    .parse(await readJson(requiredFlag(invocation, 'schedule')));
  const cases = await loadBenchmarkCases(optionalFlag(invocation, 'cases'));
  const byCase = new Map(cases.map((entry) => [entry.id, entry]));
  const family = z
    .enum(['exploratory', 'primary', 'replication', 'sensitivity'])
    .parse(requiredFlag(invocation, 'family'));
  const freeze = await loadFreezeManifest(optionalFlag(invocation, 'freeze'));
  const policy = eligibility(family, freeze);
  if (family !== 'exploratory') {
    if (freeze === undefined) throw new TypeError('freeze is required');
    validateFrozenScoreCorpus(family, freeze, cases, schedule);
  }
  const root = requiredFlag(invocation, 'artifacts');
  const events = createEventStore(root);
  const records = createRecordStore(root);
  const rows: ScoreRow[] = [];
  for (const run of [...schedule].sort(
    (left, right) => left.order - right.order,
  )) {
    const benchmarkCase = byCase.get(run.caseId);
    if (benchmarkCase === undefined) {
      throw new TypeError(`score case is unavailable: ${run.caseId}`);
    }
    const attempts = await records.read(run.id);
    if (attempts.length === 0 || resumeAction(attempts) === 'retry-technical') {
      throw new Error(`run has no terminal attempt: ${run.id}`);
    }
    const record = attempts.at(-1);
    if (
      record === undefined ||
      artifactHash(record.run) !== artifactHash(run)
    ) {
      throw new Error(`stored run differs from schedule: ${run.id}`);
    }
    const storedEvents = await events.read(run.id, record.attempt);
    const derived = await events.derive(run.id, record.attempt);
    if (artifactHash(derived) !== artifactHash(record.trace)) {
      throw new Error(`stored trace differs from execution record: ${run.id}`);
    }
    const evidence = toScoreEvidence(
      evaluateEvidence(benchmarkCase, record, storedEvents),
      record,
    );
    rows.push(
      scoreExecution({
        record,
        case: benchmarkCase,
        evidence,
        eligibility: policy,
      }),
    );
  }
  const outputPath = optionalFlag(invocation, 'output');
  const csvPath = optionalFlag(invocation, 'csv');
  if (outputPath === undefined && csvPath === undefined) return rows;
  const json =
    outputPath === undefined
      ? undefined
      : await writeJsonExclusive(outputPath, rows);
  const csvResult =
    csvPath === undefined
      ? undefined
      : await writeTextExclusive(csvPath, csv(rows));
  return {
    count: rows.length,
    ...(json === undefined ? {} : { json }),
    ...(csvResult === undefined ? {} : { csv: csvResult }),
  };
};
