import { constants } from 'node:fs';
import { access, mkdir, open, readFile, readdir } from 'node:fs/promises';
import { join, relative } from 'node:path';

import {
  AttemptReservationV1,
  ExecutionRecordV1,
  Id,
  TraceReferenceV1,
  type AttemptReservation,
  type ExecutionRecord,
  type RunSpec,
} from '../schemas/index.js';
import { artifactHash } from '../core/hash.js';
import { canonicalJson, jsonSnapshot, type JsonValue } from '../core/json.js';

export interface StoredEvent {
  readonly schemaVersion: 1;
  readonly runId: string;
  readonly attempt: number;
  readonly sequence: number;
  readonly previousHash: string | null;
  readonly payloadHash: string;
  readonly payload: JsonValue;
  readonly eventHash: string;
}

export interface EventStore {
  readonly append: (
    runId: string,
    attempt: number,
    payload: JsonValue,
  ) => Promise<StoredEvent>;
  readonly read: (
    runId: string,
    attempt: number,
  ) => Promise<readonly StoredEvent[]>;
  readonly derive: (
    runId: string,
    attempt: number,
  ) => Promise<ReturnType<typeof TraceReferenceV1.parse>>;
}

const emptyRoot = artifactHash([]);

const eventBody = (
  runId: string,
  attempt: number,
  sequence: number,
  previousHash: string | null,
  payload: JsonValue,
) => ({
  schemaVersion: 1 as const,
  runId,
  attempt,
  sequence,
  previousHash,
  payloadHash: artifactHash(payload),
  payload: jsonSnapshot(payload),
});

const eventFile = (event: StoredEvent): string =>
  `${String(event.sequence).padStart(8, '0')}-${event.eventHash.slice(7)}.json`;

const nameStartsWithAttempt = (
  name: string | undefined,
  attempt: number,
): boolean => name?.startsWith(`${String(attempt).padStart(3, '0')}-`) ?? false;

const verifyEvent = (
  event: StoredEvent,
  expectedSequence: number,
  previousHash: string | null,
  file: string,
): void => {
  const { eventHash, ...body } = event;
  if (
    event.sequence !== expectedSequence ||
    event.previousHash !== previousHash
  ) {
    throw new Error('trace chain sequence mismatch');
  }
  if (
    artifactHash(event.payload) !== event.payloadHash ||
    artifactHash(body) !== eventHash
  ) {
    throw new Error('trace content hash mismatch');
  }
  if (file !== eventFile(event))
    throw new Error('trace filename hash mismatch');
};

const isMissing = (error: unknown): boolean =>
  typeof error === 'object' &&
  error !== null &&
  'code' in error &&
  error.code === 'ENOENT';

const isExisting = (error: unknown): boolean =>
  typeof error === 'object' &&
  error !== null &&
  'code' in error &&
  error.code === 'EEXIST';

const readJson = async <T>(path: string): Promise<T> =>
  JSON.parse(await readFile(path, 'utf8')) as T;

const writeExclusive = async (path: string, content: string): Promise<void> => {
  const handle = await open(path, 'wx', 0o600);
  try {
    await handle.writeFile(content, 'utf8');
    await handle.sync();
  } finally {
    await handle.close();
  }
};

const assertRunId = (runId: string): string => Id.parse(runId);
const assertAttempt = (attempt: number): number => {
  if (!Number.isSafeInteger(attempt) || attempt < 1)
    throw new TypeError('attempt must be a positive safe integer');
  return attempt;
};

const createLocks = () => {
  const tails = new Map<string, Promise<void>>();
  return async <T>(key: string, operation: () => Promise<T>): Promise<T> => {
    const previous = tails.get(key) ?? Promise.resolve();
    let release = (): void => undefined;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const tail = previous.then(() => gate);
    tails.set(key, tail);
    await previous;
    try {
      return await operation();
    } finally {
      release();
      if (tails.get(key) === tail) tails.delete(key);
    }
  };
};

/** Creates an append-only, content-addressed per-event trace store. */
export const createEventStore = (root: string): EventStore => {
  const lock = createLocks();
  const attemptDirectory = (runId: string, attempt: number) =>
    join(
      root,
      'traces',
      assertRunId(runId),
      `attempt-${String(assertAttempt(attempt)).padStart(3, '0')}`,
    );
  const eventDirectory = (runId: string, attempt: number) =>
    join(attemptDirectory(runId, attempt), 'events');

  const read = async (
    runId: string,
    attempt: number,
  ): Promise<readonly StoredEvent[]> => {
    const directory = eventDirectory(runId, attempt);
    let entries;
    try {
      entries = await readdir(directory, { withFileTypes: true });
    } catch (error) {
      if (isMissing(error)) return [];
      throw error;
    }
    const files = entries
      .filter((entry) => entry.isFile() && entry.name.endsWith('.json'))
      .map((entry) => entry.name)
      .sort();
    const events = await Promise.all(
      files.map((file) => readJson<StoredEvent>(join(directory, file))),
    );
    events.forEach((event, index) =>
      verifyEvent(
        event,
        index + 1,
        index === 0 ? null : (events[index - 1]?.eventHash ?? null),
        files[index] as string,
      ),
    );
    if (
      events.some((event) => event.runId !== runId || event.attempt !== attempt)
    )
      throw new Error('trace run or attempt mismatch');
    return events;
  };

  const append = async (
    runId: string,
    attempt: number,
    payload: JsonValue,
  ): Promise<StoredEvent> =>
    lock(`${assertRunId(runId)}:${assertAttempt(attempt)}`, async () => {
      const events = await read(runId, attempt);
      const body = eventBody(
        runId,
        attempt,
        events.length + 1,
        events.at(-1)?.eventHash ?? null,
        payload,
      );
      const event: StoredEvent = { ...body, eventHash: artifactHash(body) };
      const directory = eventDirectory(runId, attempt);
      await mkdir(directory, { recursive: true });
      await writeExclusive(
        join(directory, eventFile(event)),
        `${canonicalJson(event)}\n`,
      );
      return event;
    });

  const derive = async (runId: string, attempt: number) => {
    assertRunId(runId);
    assertAttempt(attempt);
    const events = await read(runId, attempt);
    const body = { schemaVersion: 1 as const, runId, attempt, events };
    const derivedHash = artifactHash(body);
    const directory = join(attemptDirectory(runId, attempt), 'derived');
    const name = `${derivedHash.slice(7)}.json`;
    const path = join(directory, name);
    await mkdir(directory, { recursive: true });
    try {
      await writeExclusive(path, `${canonicalJson(body)}\n`);
    } catch (error) {
      if (!isExisting(error)) throw error;
      const existing = await readFile(path, 'utf8');
      if (existing !== `${canonicalJson(body)}\n`)
        throw new Error('immutable derived trace conflict');
    }
    return TraceReferenceV1.parse({
      rootHash: events.at(-1)?.eventHash ?? emptyRoot,
      derivedHash,
      eventCount: events.length,
      relativePath: relative(root, path),
    });
  };

  return { append, read, derive };
};

export interface RecordStore {
  readonly reserve: (
    run: RunSpec,
    reservedAt: string,
  ) => Promise<AttemptReservation>;
  readonly reservations: (
    runId: string,
  ) => Promise<readonly AttemptReservation[]>;
  readonly pending: (runId: string) => Promise<readonly AttemptReservation[]>;
  readonly append: (record: ExecutionRecord) => Promise<string>;
  readonly read: (runId: string) => Promise<readonly ExecutionRecord[]>;
}

/** Stores every immutable execution attempt without replacing earlier outcomes. */
export const createRecordStore = (root: string): RecordStore => {
  const lock = createLocks();
  const reservationDirectory = (runId: string): string =>
    join(root, 'attempts', assertRunId(runId));
  const reservationName = (attempt: number): string =>
    `${String(assertAttempt(attempt)).padStart(8, '0')}.json`;
  const reservations = async (
    runId: string,
  ): Promise<readonly AttemptReservation[]> => {
    const directory = reservationDirectory(runId);
    let names: readonly string[];
    try {
      names = (await readdir(directory))
        .filter((name) => name.endsWith('.json'))
        .sort();
    } catch (error) {
      if (isMissing(error)) return [];
      throw error;
    }
    const values = await Promise.all(
      names.map(async (name) =>
        AttemptReservationV1.parse(
          await readJson<unknown>(join(directory, name)),
        ),
      ),
    );
    values.forEach((reservation, index) => {
      if (
        reservation.run.id !== runId ||
        names[index] !== reservationName(reservation.attempt) ||
        (index > 0 && reservation.attempt <= values[index - 1]!.attempt)
      ) {
        throw new Error('attempt reservation sequence mismatch');
      }
    });
    return values;
  };
  const read = async (runId: string): Promise<readonly ExecutionRecord[]> => {
    const directory = join(root, 'records', assertRunId(runId));
    let names: readonly string[];
    try {
      names = (await readdir(directory))
        .filter((name) => name.endsWith('.json'))
        .sort();
    } catch (error) {
      if (isMissing(error)) return [];
      throw error;
    }
    const records = await Promise.all(
      names.map(async (name) =>
        ExecutionRecordV1.parse(await readJson<unknown>(join(directory, name))),
      ),
    );
    records.forEach((record, index) => {
      if (
        record.run.id !== runId ||
        record.attempt !== index + 1 ||
        !nameStartsWithAttempt(names[index], record.attempt)
      ) {
        throw new Error('record attempt sequence mismatch');
      }
    });
    return records;
  };
  const reserve = async (
    run: RunSpec,
    reservedAt: string,
  ): Promise<AttemptReservation> =>
    lock(assertRunId(run.id), async () => {
      const [attempts, reserved] = await Promise.all([
        read(run.id),
        reservations(run.id),
      ]);
      const attempt =
        Math.max(attempts.at(-1)?.attempt ?? 0, reserved.at(-1)?.attempt ?? 0) +
        1;
      const reservation = AttemptReservationV1.parse({
        schemaVersion: 1,
        attempt,
        run,
        reservedAt,
      });
      const directory = reservationDirectory(run.id);
      await mkdir(directory, { recursive: true });
      await writeExclusive(
        join(directory, reservationName(attempt)),
        `${canonicalJson(reservation)}\n`,
      );
      return reservation;
    });
  const pending = async (
    runId: string,
  ): Promise<readonly AttemptReservation[]> => {
    const [reserved, attempts] = await Promise.all([
      reservations(runId),
      read(runId),
    ]);
    const terminal = new Map(
      attempts.map((record) => [record.attempt, record]),
    );
    for (const reservation of reserved) {
      const record = terminal.get(reservation.attempt);
      if (
        record !== undefined &&
        artifactHash(record.run) !== artifactHash(reservation.run)
      ) {
        throw new Error('attempt reservation run mismatch');
      }
    }
    return reserved.filter(({ attempt }) => !terminal.has(attempt));
  };
  const append = async (record: ExecutionRecord): Promise<string> =>
    lock(assertRunId(record.run.id), async () => {
      const attempts = await read(record.run.id);
      const expectedAttempt = attempts.length + 1;
      if (record.attempt !== expectedAttempt)
        throw new TypeError(
          'record attempt is not the next immutable sequence',
        );
      const directory = join(root, 'records', record.run.id);
      const reservation = (await reservations(record.run.id)).find(
        ({ attempt }) => attempt === record.attempt,
      );
      if (
        reservation !== undefined &&
        artifactHash(reservation.run) !== artifactHash(record.run)
      ) {
        throw new TypeError('record does not match its attempt reservation');
      }
      const hash = artifactHash(record);
      const name = `${String(record.attempt).padStart(3, '0')}-${hash.slice(7)}.json`;
      await mkdir(directory, { recursive: true });
      const path = join(directory, name);
      await writeExclusive(path, `${canonicalJson(record)}\n`);
      return path;
    });
  return { reserve, reservations, pending, append, read };
};

/** Checks whether a frozen file already exists without mutating it. */
export const immutableExists = async (path: string): Promise<boolean> =>
  access(path, constants.F_OK).then(
    () => true,
    (error: unknown) => {
      if (isMissing(error)) return false;
      throw error;
    },
  );
