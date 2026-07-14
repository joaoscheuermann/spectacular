import { createHash } from 'node:crypto';
import { appendFile, mkdir, readFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';

import { assertSafeWritePath } from './layout.js';
import type { FailedEval } from './evaluate.js';
import type {
  EvalAssertion,
  ModelRef,
  Scenario,
  TargetModel,
} from './schema.js';

export type OptimizerMode = 'normal' | 'plateau-escape' | 'compression';
export type HistoryDisposition =
  | 'improved'
  | 'not-improved'
  | 'duplicate-prompt'
  | 'rejected-strategy'
  | 'compression-accepted'
  | 'compression-rejected';
export type TerminalStatus =
  'approved' | 'validation-failed' | 'plateau' | 'max-epochs';

export type HistoryRecord = {
  readonly fingerprint: string;
  readonly attemptedPrompt: string;
  readonly strategy: string;
  readonly optimizerMode: OptimizerMode;
  readonly trainingAccuracy: number;
  readonly failedEvals: readonly FailedEval[];
  readonly disposition: HistoryDisposition;
  readonly terminalStatus: TerminalStatus | null;
};

export type OptimizerHistory = Omit<
  HistoryRecord,
  'fingerprint' | 'terminalStatus'
>;

type FingerprintInputs = {
  readonly originalPrompt: string;
  readonly training: readonly Scenario[];
  readonly globalEvals: readonly EvalAssertion[];
  readonly accuracy: number;
  readonly target: TargetModel;
  readonly judge: ModelRef;
};

const evaluationMode = 'per-eval-sample-v1';

/** Identifies history that is valid for one immutable training contract. */
export const historyFingerprint = (inputs: FingerprintInputs): string =>
  createHash('sha256')
    .update(
      JSON.stringify({
        evaluationMode,
        originalPrompt: inputs.originalPrompt,
        training: inputs.training,
        globalEvals: inputs.globalEvals,
        accuracy: inputs.accuracy,
        target: inputs.target,
        judge: inputs.judge,
      }),
    )
    .digest('hex');

export const historyPath = (root: string, targetId: string): string =>
  join(root, targetId, 'evolution.history.jsonl');

const exactKeys = (value: object, keys: readonly string[]): boolean => {
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  return (
    actual.length === keys.length &&
    actual.every((key, index) => key === expected[index])
  );
};

const isFailedEval = (value: unknown): value is FailedEval => {
  if (typeof value !== 'object' || value === null) return false;
  const failure = value as Partial<FailedEval>;
  return (
    exactKeys(value, [
      'evalId',
      'output',
      'passed',
      'reasoning',
      'sampleIndex',
      'scenarioId',
    ]) &&
    typeof failure.evalId === 'string' &&
    failure.evalId.trim() !== '' &&
    typeof failure.sampleIndex === 'number' &&
    Number.isInteger(failure.sampleIndex) &&
    failure.sampleIndex >= 0 &&
    failure.sampleIndex <= 2 &&
    typeof failure.reasoning === 'string' &&
    failure.reasoning.trim() !== '' &&
    failure.passed === false &&
    typeof failure.scenarioId === 'string' &&
    failure.scenarioId.trim() !== '' &&
    typeof failure.output === 'string'
  );
};

const isRecord = (value: unknown): value is HistoryRecord => {
  if (typeof value !== 'object' || value === null) return false;
  const record = value as Partial<HistoryRecord>;
  const modes: readonly string[] = ['normal', 'plateau-escape', 'compression'];
  const dispositions: readonly string[] = [
    'improved',
    'not-improved',
    'duplicate-prompt',
    'rejected-strategy',
    'compression-accepted',
    'compression-rejected',
  ];
  const terminals: readonly unknown[] = [
    null,
    'approved',
    'validation-failed',
    'plateau',
    'max-epochs',
  ];
  return (
    exactKeys(value, [
      'attemptedPrompt',
      'disposition',
      'failedEvals',
      'fingerprint',
      'optimizerMode',
      'strategy',
      'terminalStatus',
      'trainingAccuracy',
    ]) &&
    typeof record.fingerprint === 'string' &&
    typeof record.attemptedPrompt === 'string' &&
    typeof record.strategy === 'string' &&
    typeof record.optimizerMode === 'string' &&
    modes.includes(record.optimizerMode) &&
    typeof record.trainingAccuracy === 'number' &&
    Number.isFinite(record.trainingAccuracy) &&
    Array.isArray(record.failedEvals) &&
    record.failedEvals.every(isFailedEval) &&
    typeof record.disposition === 'string' &&
    dispositions.includes(record.disposition) &&
    terminals.includes(record.terminalStatus)
  );
};

/** Removes fingerprint and terminal information before optimizer reuse. */
export const optimizerHistory = (
  records: readonly HistoryRecord[],
): readonly OptimizerHistory[] =>
  records.map(
    ({
      attemptedPrompt,
      strategy,
      optimizerMode,
      trainingAccuracy,
      failedEvals,
      disposition,
    }) => ({
      attemptedPrompt,
      strategy,
      optimizerMode,
      trainingAccuracy,
      failedEvals,
      disposition,
    }),
  );

/** Preflights the durable history destination without mutating it. */
export const preflightHistory = (
  root: string,
  targetId: string,
): Promise<void> => assertSafeWritePath(root, historyPath(root, targetId));

/** Reads only the newest matching attempts; absent history is an empty list. */
export const loadHistory = async (
  root: string,
  targetId: string,
  fingerprint: string,
  limit: number,
): Promise<readonly HistoryRecord[]> => {
  const path = historyPath(root, targetId);
  await preflightHistory(root, targetId);
  let body: string;
  try {
    body = await readFile(path, 'utf8');
  } catch (error) {
    if (error instanceof Error && 'code' in error && error.code === 'ENOENT') {
      return [];
    }
    throw error;
  }
  const records = body
    .split('\n')
    .filter((line) => line.trim() !== '')
    .map((line) => JSON.parse(line) as unknown);
  if (!records.every(isRecord)) {
    throw new Error(`Invalid evolution history for target "${targetId}".`);
  }
  return records
    .filter((record) => record.fingerprint === fingerprint)
    .slice(-limit);
};

/** Appends attempts after checking the target path for symlink escapes. */
export const appendHistory = async (
  root: string,
  targetId: string,
  records: readonly HistoryRecord[],
): Promise<void> => {
  if (records.length === 0) return;
  const path = historyPath(root, targetId);
  await preflightHistory(root, targetId);
  await mkdir(dirname(path), { recursive: true });
  await appendFile(
    path,
    records.map((record) => JSON.stringify(record) + '\n').join(''),
    'utf8',
  );
};
