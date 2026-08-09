import { execFile } from 'node:child_process';
import { promisify } from 'node:util';

import { z } from 'zod';

import { HARNESS_COMMANDS } from '../core/commands.js';
import { artifactHash } from '../core/index.js';
import {
  FreezeManifestV1,
  PowerResultV1,
  RunSpecV1,
  ScoreRowV1,
  type FreezeManifest,
  type ScoreRow,
} from '../schemas/index.js';
import { modelCallBudgetAtP95 } from '../scoring/index.js';
import {
  PILOT_CASES,
  confirmatoryCasesHash,
  localInstrumentHashes,
  seedLedgerHash,
  selectBaseline,
  validateConfirmatoryCases,
  type FreezeInput,
} from '../study/index.js';
import type { CliInvocation } from './args.js';
import { rejectUnknownFlags, requiredFlag } from './args.js';
import { objectValue, readJson } from './io.js';
import { parsePrices, pricesHash } from './pricing.js';
import { validatePriceCoverage } from './pricing.js';
import { loadProductionIndex } from './index-lifecycle.js';
import { loadBenchmarkCases } from './run.js';
import { fileHash, imageDigest, input } from './shared.js';
import { verifiedCalibration } from './calibration.js';

const executeFile = promisify(execFile);

export const validatePilotFreeze = (
  rows: readonly ScoreRow[],
  proposed: FreezeManifest,
): void => {
  const conditionIds = ['B0', 'B1', 'B2', 'B3', 'M0', 'M1'] as const;
  const baselineIds: ReadonlySet<string> = new Set(conditionIds.slice(0, 4));
  const pilotCaseIds = new Set(PILOT_CASES.map(({ id }) => id));
  const cells = new Set(
    rows.map(
      ({ caseId, conditionId, repetition }) =>
        `${caseId}\r${conditionId}\r${repetition}`,
    ),
  );
  const baselines = rows.filter(({ conditionId }) =>
    baselineIds.has(conditionId),
  );
  const counts = conditionIds.map(
    (conditionId) =>
      rows.filter((row) => row.conditionId === conditionId).length,
  );
  const complete = PILOT_CASES.every(({ id }) =>
    conditionIds.every((conditionId) =>
      Array.from({ length: 5 }, (_, index) => index + 1).every((repetition) =>
        cells.has(`${id}\r${conditionId}\r${repetition}`),
      ),
    ),
  );
  if (
    rows.length !== 1_800 ||
    baselines.length !== 1_200 ||
    counts.some((count) => count !== 300) ||
    cells.size !== rows.length ||
    !complete ||
    new Set(rows.map(({ runId }) => runId)).size !== rows.length ||
    rows.some(
      ({
        studyId,
        phase,
        caseId,
        repetition,
        pairedBlock,
        provider,
        model,
        effort,
        freezeHash,
        modelCallBudget,
        primaryEligible,
      }) =>
        studyId !== proposed.studyId ||
        phase !== 'pilot' ||
        !pilotCaseIds.has(caseId) ||
        pairedBlock !== `${caseId}.rep.${repetition}` ||
        provider !== proposed.primaryModel.provider ||
        model !== proposed.primaryModel.model ||
        effort !== proposed.primaryModel.effort ||
        freezeHash !== null ||
        modelCallBudget !== null ||
        primaryEligible,
    )
  ) {
    throw new TypeError('pilot scores are not the complete frozen pilot');
  }
  const selected = selectBaseline(
    baselines.map(({ conditionId, success, costUsd }) => ({
      conditionId: conditionId as 'B0' | 'B1' | 'B2' | 'B3',
      success,
      costUsd,
    })),
  );
  if (selected.conditionId !== proposed.selectedBaseline) {
    throw new TypeError(
      'selected baseline differs from the frozen tie-break rule',
    );
  }
  if (
    modelCallBudgetAtP95(rows, selected.conditionId) !==
    proposed.modelCallBudgetP95
  ) {
    throw new TypeError('p95 model-call budget differs from the pilot');
  }
};

export const validateFreezeSchedule = (
  proposed: FreezeManifest,
  cases: Awaited<ReturnType<typeof loadBenchmarkCases>>,
  schedule: readonly z.infer<typeof RunSpecV1>[],
): void => {
  const conditions = new Set([proposed.selectedBaseline, 'M1']);
  const keys = new Set(
    schedule.map(
      ({ caseId, conditionId, repetition }) =>
        `${caseId}\r${conditionId}\r${repetition}`,
    ),
  );
  const complete = cases.every(({ id }) =>
    [...conditions].every((conditionId) =>
      Array.from(
        { length: proposed.repetitions },
        (_, index) => index + 1,
      ).every((repetition) => keys.has(`${id}\r${conditionId}\r${repetition}`)),
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
    schedule.length !== proposed.nFinal * proposed.repetitions * 2 ||
    keys.size !== schedule.length ||
    !complete ||
    pairingMismatch ||
    new Set(schedule.map(({ pairedBlock }) => pairedBlock)).size !==
      proposed.nFinal * proposed.repetitions ||
    new Set(schedule.map(({ order }) => order)).size !== schedule.length ||
    Math.min(...schedule.map(({ order }) => order)) !== 0 ||
    Math.max(...schedule.map(({ order }) => order)) !== schedule.length - 1 ||
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
        studyId !== proposed.studyId ||
        phase !== 'confirmatory' ||
        !conditions.has(conditionId) ||
        artifactHash(model) !== artifactHash(proposed.primaryModel) ||
        capture !== 'structure' ||
        (freezeHash !== null && freezeHash !== proposed.manifestHash) ||
        modelCallBudget !== undefined ||
        oracleParentRunId !== undefined,
    )
  ) {
    throw new TypeError(
      'confirmatory schedule is not the complete paired design',
    );
  }
};

export const freeze = async (invocation: CliInvocation): Promise<unknown> => {
  rejectUnknownFlags(invocation, [
    'input',
    'path',
    'pilot-scores',
    'calibration',
    'calibration-audit',
    'confirmatory-audit',
    'cost-approval',
    'power-config',
    'power-approval',
    'power-result',
    'cases',
    'schedule',
    'prices',
    'index',
    'image',
  ]);
  const raw = objectValue(await input(invocation), 'freeze input');
  if ('manifestHash' in raw) {
    throw new TypeError('freeze input must not declare manifestHash');
  }
  const proposed = FreezeManifestV1.parse({
    ...raw,
    manifestHash: artifactHash(raw),
  });
  const pilotScores = z
    .array(ScoreRowV1)
    .parse(await readJson(requiredFlag(invocation, 'pilot-scores')));
  validatePilotFreeze(pilotScores, proposed);
  if (artifactHash(pilotScores) !== proposed.artifactHashes.pilotScores) {
    throw new Error('frozen pilot score hash mismatch');
  }

  const calibration = await verifiedCalibration(
    requiredFlag(invocation, 'calibration'),
  );
  if (
    calibration.studyId !== proposed.studyId ||
    calibration.artifactHash !== proposed.artifactHashes.calibration ||
    artifactHash({
      candidate: calibration.candidate,
      ...calibration.result,
    }) !== artifactHash(proposed.replication)
  ) {
    throw new Error('frozen replication calibration mismatch');
  }
  const [calibrationAuditHash, confirmatoryAuditHash, costApprovalHash] =
    await Promise.all([
      fileHash(requiredFlag(invocation, 'calibration-audit')),
      fileHash(requiredFlag(invocation, 'confirmatory-audit')),
      fileHash(requiredFlag(invocation, 'cost-approval')),
    ]);
  if (
    calibrationAuditHash !== proposed.artifactHashes.calibrationAudit ||
    confirmatoryAuditHash !== proposed.artifactHashes.confirmatoryAudit ||
    costApprovalHash !== proposed.artifactHashes.costApproval
  ) {
    throw new Error('frozen human approval hash mismatch');
  }

  const powerConfigPath = requiredFlag(invocation, 'power-config');
  const powerApprovalPath = requiredFlag(invocation, 'power-approval');
  const powerResultPath = requiredFlag(invocation, 'power-result');
  const powerConfigHash = await fileHash(powerConfigPath);
  const powerApprovalHash = await fileHash(powerApprovalPath);
  const powerResult = PowerResultV1.parse(await readJson(powerResultPath));
  if (
    powerConfigHash !== proposed.artifactHashes.powerConfig ||
    powerResult.configHash !== powerConfigHash ||
    powerApprovalHash !== proposed.artifactHashes.powerApproval ||
    (await fileHash(powerResultPath)) !== proposed.artifactHashes.powerResult ||
    powerResult.nPower !== proposed.nPower ||
    powerResult.nFinal !== proposed.nFinal ||
    powerResult.minimumEffect !== proposed.minimumEffect ||
    powerResult.power < proposed.targetPower
  ) {
    throw new Error('frozen power result mismatch');
  }

  const cases = await loadBenchmarkCases(requiredFlag(invocation, 'cases'));
  const caseIssues = validateConfirmatoryCases(
    cases,
    PILOT_CASES,
    proposed.nFinal,
  );
  if (caseIssues.length > 0) {
    throw new TypeError(
      `confirmatory cases are invalid: ${caseIssues[0]?.code ?? 'unknown'}`,
    );
  }
  if (
    confirmatoryCasesHash(cases) !== proposed.artifactHashes.confirmatoryCases
  ) {
    throw new Error('frozen confirmatory case hash mismatch');
  }

  const schedule = z
    .array(RunSpecV1)
    .parse(await readJson(requiredFlag(invocation, 'schedule')));
  validateFreezeSchedule(proposed, cases, schedule);
  if (seedLedgerHash(schedule) !== proposed.artifactHashes.seeds) {
    throw new Error('frozen paired seed ledger hash mismatch');
  }

  const prices = parsePrices(
    await readJson(requiredFlag(invocation, 'prices')),
  );
  validatePriceCoverage(prices, [
    { model: proposed.primaryModel.model, kind: 'completion' },
    { model: proposed.replication.candidate.model, kind: 'completion' },
    { model: proposed.embedder.model, kind: 'embedding' },
    { model: proposed.reranker, kind: 'rerank' },
  ]);
  if (pricesHash(prices) !== proposed.artifactHashes.prices) {
    throw new Error('frozen price hash mismatch');
  }
  const index = await loadProductionIndex(requiredFlag(invocation, 'index'));
  if (index.indexHash !== proposed.artifactHashes.retrievalIndex) {
    throw new Error('frozen retrieval index hash mismatch');
  }
  const localHashes = await localInstrumentHashes();
  for (const name of Object.keys(localHashes) as (keyof typeof localHashes)[]) {
    if (localHashes[name] !== proposed.artifactHashes[name]) {
      throw new Error(`frozen ${name} hash differs from the local instrument`);
    }
  }

  const image = requiredFlag(invocation, 'image');
  if (imageDigest(image) !== proposed.containerDigest) {
    throw new Error('frozen OCI image digest mismatch');
  }
  await executeFile('docker', ['image', 'inspect', image]).catch(() => {
    throw new Error('frozen OCI image is unavailable locally');
  });
  const [status, commit] = await Promise.all([
    executeFile('git', ['status', '--porcelain=v1', '--untracked-files=all']),
    executeFile('git', ['rev-parse', 'HEAD']),
  ]);
  if (
    status.stdout.trim().length > 0 ||
    commit.stdout.trim() !== proposed.gitCommit
  ) {
    throw new Error('freeze requires the recorded clean git commit');
  }
  const { manifestHash: _manifestHash, ...freezeInput } = proposed;
  return HARNESS_COMMANDS.freeze(
    requiredFlag(invocation, 'path'),
    freezeInput as FreezeInput,
  );
};
