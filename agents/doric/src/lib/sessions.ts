import { randomUUID } from 'node:crypto';

import {
  MosaicSessionState,
  Prisma,
  type MosaicEvent as StoredEvent,
  type MosaicSession as StoredSession,
} from '../generated/prisma/client.js';
import type { DoricConfig } from './config.js';
import type { Database } from './database.js';

export type SessionState =
  | 'queued'
  | 'running'
  | 'cancelling'
  | 'completed'
  | 'failed'
  | 'cancelled';

export type Session = {
  readonly id: string;
  readonly prompt: string;
  readonly state: SessionState;
  readonly configRevision: number;
  readonly result: unknown;
  readonly errorCode?: string;
  readonly lastSequence: number;
  readonly createdAt: string;
  readonly updatedAt: string;
  readonly startedAt?: string;
  readonly finishedAt?: string;
};

export type SessionEvent = {
  readonly sessionId: string;
  readonly sequence: number;
  readonly type: string;
  readonly event: unknown;
  readonly createdAt: string;
};

export type SessionRecord = {
  readonly session: Session;
  readonly snapshot: DoricConfig;
};

export type SessionStore = ReturnType<typeof createSessionStore>;

const terminal = new Set<MosaicSessionState>([
  MosaicSessionState.COMPLETED,
  MosaicSessionState.FAILED,
  MosaicSessionState.CANCELLED,
]);

/** Owns transactional session state and durable, contiguous Mosaic events. */
export const createSessionStore = (database: Database) => ({
  async reconcile(): Promise<number> {
    const result = await database.mosaicSession.updateMany({
      where: { state: { notIn: [...terminal] } },
      data: {
        state: MosaicSessionState.FAILED,
        errorCode: 'process_interrupted',
        finishedAt: new Date(),
      },
    });
    return result.count;
  },

  async create(prompt: string, snapshot: DoricConfig): Promise<SessionRecord> {
    const stored = await database.mosaicSession.create({
      data: {
        id: randomUUID(),
        prompt,
        configRevision: snapshot.revision,
        configSnapshot: json(snapshot),
      },
    });
    return { session: session(stored), snapshot };
  },

  async find(id: string): Promise<SessionRecord | undefined> {
    const stored = await database.mosaicSession.findUnique({ where: { id } });
    return stored === null
      ? undefined
      : {
          session: session(stored),
          snapshot: stored.configSnapshot as unknown as DoricConfig,
        };
  },

  async list(limit: number, cursor?: string) {
    const records = await database.mosaicSession.findMany({
      take: limit + 1,
      ...(cursor === undefined ? {} : { cursor: { id: cursor }, skip: 1 }),
      orderBy: [{ createdAt: 'desc' }, { id: 'desc' }],
    });
    const hasMore = records.length > limit;
    const page = records.slice(0, limit).map(session);
    return {
      sessions: page,
      nextCursor: hasMore ? page.at(-1)?.id : undefined,
    };
  },

  async markRunning(id: string): Promise<Session | undefined> {
    await database.mosaicSession.updateMany({
      where: { id, state: MosaicSessionState.QUEUED },
      data: { state: MosaicSessionState.RUNNING, startedAt: new Date() },
    });
    const stored = await database.mosaicSession.findUnique({ where: { id } });
    return stored === null ? undefined : session(stored);
  },

  async requestCancellation(id: string): Promise<Session | undefined> {
    return database.$transaction(async (transaction) => {
      const current = await transaction.mosaicSession.findUnique({
        where: { id },
      });
      if (current === null || terminal.has(current.state)) {
        return current === null ? undefined : session(current);
      }
      if (current.state === MosaicSessionState.CANCELLING)
        return session(current);
      return session(
        await transaction.mosaicSession.update({
          where: { id },
          data: { state: MosaicSessionState.CANCELLING },
        }),
      );
    });
  },

  async finish(
    id: string,
    target: 'completed' | 'failed' | 'cancelled',
    result?: unknown,
    errorCode?: string,
  ): Promise<Session | undefined> {
    return database.$transaction(async (transaction) => {
      const current = await transaction.mosaicSession.findUnique({
        where: { id },
      });
      if (current === null) return undefined;
      if (terminal.has(current.state)) return session(current);
      const state =
        current.state === MosaicSessionState.CANCELLING
          ? MosaicSessionState.CANCELLED
          : toStoredState(target);
      return session(
        await transaction.mosaicSession.update({
          where: { id },
          data: {
            state,
            finishedAt: new Date(),
            ...(state === MosaicSessionState.COMPLETED
              ? { result: json(result ?? null), errorCode: null }
              : {
                  errorCode:
                    errorCode ??
                    (state === MosaicSessionState.CANCELLED
                      ? null
                      : 'execution_failed'),
                }),
          },
        }),
      );
    });
  },

  async appendEvent(id: string, value: unknown): Promise<SessionEvent> {
    return database.$transaction(
      async (transaction) => {
        const current = await transaction.mosaicSession.findUniqueOrThrow({
          where: { id },
          select: { lastSequence: true },
        });
        const sequence = current.lastSequence + 1;
        const updated = await transaction.mosaicSession.updateMany({
          where: { id, lastSequence: current.lastSequence },
          data: { lastSequence: sequence },
        });
        if (updated.count !== 1)
          throw new Error('Concurrent Mosaic event sequence update.');
        const event = value as {
          readonly type?: unknown;
          readonly sequence?: unknown;
        };
        if (event.sequence !== undefined && event.sequence !== sequence) {
          throw new Error('Mosaic event sequence does not match storage.');
        }
        const stored = await transaction.mosaicEvent.create({
          data: {
            sessionId: id,
            sequence,
            type: typeof event.type === 'string' ? event.type : 'unknown',
            event: json(value),
          },
        });
        return sessionEvent(stored);
      },
      { isolationLevel: 'Serializable' },
    );
  },

  async eventsAfter(
    id: string,
    sequence: number,
  ): Promise<readonly SessionEvent[]> {
    return (
      await database.mosaicEvent.findMany({
        where: { sessionId: id, sequence: { gt: sequence } },
        orderBy: { sequence: 'asc' },
      })
    ).map(sessionEvent);
  },

  async delete(id: string): Promise<'deleted' | 'active' | 'missing'> {
    return database.$transaction(async (transaction) => {
      const current = await transaction.mosaicSession.findUnique({
        where: { id },
        select: { state: true },
      });
      if (current === null) return 'missing';
      if (!terminal.has(current.state)) return 'active';
      await transaction.mosaicSession.delete({ where: { id } });
      return 'deleted';
    });
  },
});

const json = (value: unknown): Prisma.InputJsonValue =>
  JSON.parse(JSON.stringify(value)) as Prisma.InputJsonValue;

const states: Record<MosaicSessionState, SessionState> = {
  QUEUED: 'queued',
  RUNNING: 'running',
  CANCELLING: 'cancelling',
  COMPLETED: 'completed',
  FAILED: 'failed',
  CANCELLED: 'cancelled',
};

const toStoredState = (state: 'completed' | 'failed' | 'cancelled') =>
  ({
    completed: MosaicSessionState.COMPLETED,
    failed: MosaicSessionState.FAILED,
    cancelled: MosaicSessionState.CANCELLED,
  })[state];

const session = (stored: StoredSession): Session => ({
  id: stored.id,
  prompt: stored.prompt,
  state: states[stored.state],
  configRevision: stored.configRevision,
  result: stored.result,
  ...(stored.errorCode === null ? {} : { errorCode: stored.errorCode }),
  lastSequence: stored.lastSequence,
  createdAt: stored.createdAt.toISOString(),
  updatedAt: stored.updatedAt.toISOString(),
  ...(stored.startedAt === null
    ? {}
    : { startedAt: stored.startedAt.toISOString() }),
  ...(stored.finishedAt === null
    ? {}
    : { finishedAt: stored.finishedAt.toISOString() }),
});

const sessionEvent = (stored: StoredEvent): SessionEvent => ({
  sessionId: stored.sessionId,
  sequence: stored.sequence,
  type: stored.type,
  event: stored.event,
  createdAt: stored.createdAt.toISOString(),
});
