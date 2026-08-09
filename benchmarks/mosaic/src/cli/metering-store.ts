import { mkdir, open, readFile, readdir } from 'node:fs/promises';
import { join } from 'node:path';

import type { AttemptReservation } from '../schemas/index.js';
import { artifactHash, canonicalJson } from '../core/index.js';
import type { MeterOperation, MeterStage, MeterUsage } from './provider.js';

export interface MeterEntry {
  readonly schemaVersion: 1;
  readonly scopeId: string;
  readonly attempt: number;
  readonly sequence: number;
  readonly previousHash: string | null;
  readonly operation: MeterOperation;
  readonly stage: MeterStage;
  readonly model: string;
  readonly usage: MeterUsage;
  readonly costUsd: number;
  readonly entryHash: string;
}

export interface MeterScope {
  readonly id: string;
  readonly attempt: number;
}

export interface MeteringStore {
  readonly append: (
    scope: MeterScope,
    entry: Omit<
      MeterEntry,
      | 'schemaVersion'
      | 'scopeId'
      | 'attempt'
      | 'sequence'
      | 'previousHash'
      | 'entryHash'
    >,
  ) => Promise<MeterEntry>;
  readonly read: (scope: MeterScope) => Promise<readonly MeterEntry[]>;
  readonly usage: (scope: MeterScope) => Promise<MeterUsage>;
}

const emptyUsage = (): MeterUsage => ({
  inputTokens: 0,
  outputTokens: 0,
  cachedInputTokens: 0,
  reasoningTokens: 0,
  embeddingInputTokens: 0,
  rerankInputTokens: 0,
  rerankDocuments: 0,
  rerankSearchUnits: 0,
  completionRequests: 0,
  embeddingRequests: 0,
  rerankRequests: 0,
  costUsd: 0,
});

const addUsage = (left: MeterUsage, right: MeterUsage): MeterUsage => ({
  inputTokens: left.inputTokens + right.inputTokens,
  outputTokens: left.outputTokens + right.outputTokens,
  cachedInputTokens:
    (left.cachedInputTokens ?? 0) + (right.cachedInputTokens ?? 0),
  reasoningTokens: (left.reasoningTokens ?? 0) + (right.reasoningTokens ?? 0),
  embeddingInputTokens: left.embeddingInputTokens + right.embeddingInputTokens,
  rerankInputTokens: left.rerankInputTokens + right.rerankInputTokens,
  rerankDocuments: left.rerankDocuments + right.rerankDocuments,
  rerankSearchUnits: left.rerankSearchUnits + right.rerankSearchUnits,
  completionRequests: left.completionRequests + right.completionRequests,
  embeddingRequests: left.embeddingRequests + right.embeddingRequests,
  rerankRequests: left.rerankRequests + right.rerankRequests,
  costUsd: left.costUsd + right.costUsd,
});

const isMissing = (error: unknown): boolean =>
  typeof error === 'object' &&
  error !== null &&
  'code' in error &&
  error.code === 'ENOENT';

const assertScope = (scope: MeterScope): MeterScope => {
  if (!/^[A-Za-z0-9][A-Za-z0-9._:-]*$/u.test(scope.id)) {
    throw new TypeError('meter scope ID is invalid');
  }
  if (!Number.isSafeInteger(scope.attempt) || scope.attempt < 1) {
    throw new TypeError('meter attempt must be a positive safe integer');
  }
  return scope;
};

const fileName = (entry: MeterEntry): string =>
  `${String(entry.sequence).padStart(8, '0')}-${entry.entryHash.slice(7)}.json`;

const writeExclusive = async (path: string, content: string): Promise<void> => {
  const handle = await open(path, 'wx', 0o600);
  try {
    await handle.writeFile(content, 'utf8');
    await handle.sync();
  } finally {
    await handle.close();
  }
};

/** Persists allowlisted per-response usage in an immutable hash chain. */
export const createMeteringStore = (root: string): MeteringStore => {
  const tails = new Map<string, Promise<void>>();
  const directory = (raw: MeterScope): string => {
    const scope = assertScope(raw);
    return join(
      root,
      'metering',
      scope.id,
      `attempt-${String(scope.attempt).padStart(3, '0')}`,
    );
  };
  const read = async (scope: MeterScope): Promise<readonly MeterEntry[]> => {
    const path = directory(scope);
    let names: readonly string[];
    try {
      names = (await readdir(path))
        .filter((name) => name.endsWith('.json'))
        .sort();
    } catch (error) {
      if (isMissing(error)) return [];
      throw error;
    }
    const entries = await Promise.all(
      names.map(
        async (name) =>
          JSON.parse(await readFile(join(path, name), 'utf8')) as MeterEntry,
      ),
    );
    entries.forEach((entry, index) => {
      const { entryHash, ...body } = entry;
      if (
        entry.scopeId !== scope.id ||
        entry.attempt !== scope.attempt ||
        entry.sequence !== index + 1 ||
        entry.previousHash !== (entries[index - 1]?.entryHash ?? null) ||
        artifactHash(body) !== entryHash ||
        names[index] !== fileName(entry)
      ) {
        throw new Error('metering journal integrity mismatch');
      }
    });
    return entries;
  };
  const append: MeteringStore['append'] = async (scope, value) => {
    const key = `${scope.id}:${scope.attempt}`;
    const previous = tails.get(key) ?? Promise.resolve();
    let release = (): void => undefined;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const tail = previous.then(() => gate);
    tails.set(key, tail);
    await previous;
    try {
      const entries = await read(scope);
      const body = {
        schemaVersion: 1 as const,
        scopeId: scope.id,
        attempt: scope.attempt,
        sequence: entries.length + 1,
        previousHash: entries.at(-1)?.entryHash ?? null,
        ...value,
      };
      const entry: MeterEntry = {
        ...body,
        entryHash: artifactHash(body),
      };
      const path = directory(scope);
      await mkdir(path, { recursive: true });
      await writeExclusive(
        join(path, fileName(entry)),
        `${canonicalJson(entry)}\n`,
      );
      return entry;
    } finally {
      release();
      if (tails.get(key) === tail) tails.delete(key);
    }
  };
  const usage = async (scope: MeterScope): Promise<MeterUsage> =>
    (await read(scope)).reduce(
      (total, entry) => addUsage(total, entry.usage),
      emptyUsage(),
    );
  return { append, read, usage };
};

export const reservationScope = (
  reservation: AttemptReservation,
): MeterScope => ({ id: reservation.run.id, attempt: reservation.attempt });
