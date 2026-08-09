import { z } from 'zod';

import { HARNESS_COMMANDS } from '../core/commands.js';
import { artifactHash } from '../core/index.js';
import { PowerResultV1, RunSpecV1, ScoreRowV1 } from '../schemas/index.js';
import {
  PILOT_CASES,
  confirmatoryCasesHash,
  localInstrumentHashes,
  seedLedgerHash,
  validateConfirmatoryCases,
} from '../study/index.js';
import type { CliInvocation } from './args.js';
import { optionalFlag, rejectUnknownFlags, requiredFlag } from './args.js';
import { readJson } from './io.js';
import { parsePrices, pricesHash } from './pricing.js';
import { loadProductionIndex } from './index-lifecycle.js';
import { loadBenchmarkCases } from './run.js';
import { fileHash } from './shared.js';
import { verifiedCalibration } from './calibration.js';

export const validate = async (invocation: CliInvocation): Promise<unknown> => {
  rejectUnknownFlags(invocation, [
    'cases',
    'n-final',
    'schedule',
    'prices',
    'pilot-scores',
    'calibration',
    'calibration-audit',
    'confirmatory-audit',
    'cost-approval',
    'power-config',
    'power-approval',
    'power-result',
    'index',
  ]);
  const instrument = HARNESS_COMMANDS.validate();
  const issues = [...instrument.issues];
  const artifactHashes: Record<string, string> = {
    ...(await localInstrumentHashes()),
  };
  const casesPath = optionalFlag(invocation, 'cases');
  if (casesPath !== undefined) {
    const nFinal = Number(requiredFlag(invocation, 'n-final'));
    if (!Number.isSafeInteger(nFinal) || nFinal <= 0) {
      throw new TypeError('--n-final must be a positive safe integer');
    }
    const cases = await loadBenchmarkCases(casesPath);
    artifactHashes['confirmatoryCases'] = confirmatoryCasesHash(cases);
    issues.push(
      ...validateConfirmatoryCases(cases, PILOT_CASES, nFinal).map((issue) => ({
        surface: 'confirmatory-cases',
        code: issue.code,
        detail: `${issue.caseId}: ${issue.detail}`,
      })),
    );
  } else if (optionalFlag(invocation, 'n-final') !== undefined) {
    throw new TypeError('--n-final requires --cases');
  }
  const schedulePath = optionalFlag(invocation, 'schedule');
  if (schedulePath !== undefined) {
    artifactHashes['seeds'] = seedLedgerHash(
      z.array(RunSpecV1).parse(await readJson(schedulePath)),
    );
  }
  const pricesPath = optionalFlag(invocation, 'prices');
  if (pricesPath !== undefined) {
    artifactHashes['prices'] = pricesHash(
      parsePrices(await readJson(pricesPath)),
    );
  }
  const indexPath = optionalFlag(invocation, 'index');
  if (indexPath !== undefined) {
    artifactHashes['retrievalIndex'] = (
      await loadProductionIndex(indexPath)
    ).indexHash;
  }
  const pilotScoresPath = optionalFlag(invocation, 'pilot-scores');
  if (pilotScoresPath !== undefined) {
    artifactHashes['pilotScores'] = artifactHash(
      z.array(ScoreRowV1).parse(await readJson(pilotScoresPath)),
    );
  }
  const calibrationPath = optionalFlag(invocation, 'calibration');
  if (calibrationPath !== undefined) {
    artifactHashes['calibration'] = (
      await verifiedCalibration(calibrationPath)
    ).artifactHash;
  }
  for (const [flag, name] of [
    ['calibration-audit', 'calibrationAudit'],
    ['confirmatory-audit', 'confirmatoryAudit'],
    ['cost-approval', 'costApproval'],
  ] as const) {
    const path = optionalFlag(invocation, flag);
    if (path !== undefined) artifactHashes[name] = await fileHash(path);
  }
  const powerConfigPath = optionalFlag(invocation, 'power-config');
  const powerApprovalPath = optionalFlag(invocation, 'power-approval');
  const powerResultPath = optionalFlag(invocation, 'power-result');
  if (
    [powerConfigPath, powerApprovalPath, powerResultPath].filter(
      (value) => value !== undefined,
    ).length %
      3 !==
    0
  ) {
    throw new TypeError(
      '--power-config, --power-approval and --power-result must be supplied together',
    );
  }
  if (
    powerConfigPath !== undefined &&
    powerApprovalPath !== undefined &&
    powerResultPath !== undefined
  ) {
    const configHash = await fileHash(powerConfigPath);
    const result = PowerResultV1.parse(await readJson(powerResultPath));
    if (result.configHash !== configHash) {
      throw new Error('power result does not bind the supplied configuration');
    }
    artifactHashes['powerConfig'] = configHash;
    artifactHashes['powerApproval'] = await fileHash(powerApprovalPath);
    artifactHashes['powerResult'] = await fileHash(powerResultPath);
  }
  return {
    ...instrument,
    valid: issues.length === 0,
    issues,
    artifactHashes,
  };
};
