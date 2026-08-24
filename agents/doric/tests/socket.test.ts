import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import test from 'node:test';

import { io as connect, type Socket } from 'socket.io-client';
import { Server as SocketServer } from 'socket.io';

import { createSessionsSocket } from '../src/lib/socket.js';
import type { SessionEvent } from '../src/lib/sessions.js';

test('plays full history from connection query before live events', async () => {
  const events = [storedEvent(1), storedEvent(2)];
  const store = {
    find: async () => ({ session, snapshot: {}, messages: [] }),
    eventsAfter: async (_id: string, sequence: number) =>
      events.filter((event) => event.sequence > sequence),
  };
  const host = await socketHost(store);

  try {
    const socket = connect(host.url, {
      transports: ['websocket'],
      query: { sessionId },
    });
    const snapshot = await next<{ events: SessionEvent[] }>(
      socket,
      'session:snapshot',
    );
    assert.deepEqual(snapshot.events, events);

    const live = next<SessionEvent>(socket, 'agent:event');
    const third = storedEvent(3);
    events.push(third);
    host.publisher.event(third);
    assert.deepEqual(await live, third);
    socket.close();
  } finally {
    await host.close();
  }
});

test('uses an exclusive cursor and avoids replay/live race duplicates', async () => {
  const events = [storedEvent(1), storedEvent(2)];
  let releaseReplay: () => void = () => undefined;
  const replayGate = new Promise<void>((resolve) => {
    releaseReplay = resolve;
  });
  const store = {
    find: async () => ({ session, snapshot: {}, messages: [] }),
    eventsAfter: async (_id: string, sequence: number) => {
      await replayGate;
      return events.filter((event) => event.sequence > sequence);
    },
  };
  const host = await socketHost(store);

  try {
    const socket = connect(host.url, {
      transports: ['websocket'],
      query: { sessionId, afterSequence: 1 },
    });
    await next(socket, 'connect');
    const third = storedEvent(3);
    events.push(third);
    host.publisher.event(third);
    const unexpected: SessionEvent[] = [];
    socket.on('agent:event', (event) => unexpected.push(event));
    const snapshot = next<{ events: SessionEvent[] }>(
      socket,
      'session:snapshot',
    );
    releaseReplay();
    assert.deepEqual((await snapshot).events, [events[1], third]);
    await new Promise<void>((resolve) => setImmediate(resolve));
    assert.deepEqual(unexpected, []);
    socket.close();
  } finally {
    await host.close();
  }
});

test('rejects connections without a valid sessionId query', async () => {
  const host = await socketHost({
    find: async () => undefined,
    eventsAfter: async () => [],
  });

  try {
    const socket = connect(host.url, { transports: ['websocket'] });
    const error = await next<Error>(socket, 'connect_error');
    assert.match(error.message, /sessionId/u);
    socket.close();
  } finally {
    await host.close();
  }
});

const sessionId = '018f47d2-e3b1-7b4f-8b2c-1f5a7fdf1601';
const promptId = '018f47d2-e3b1-7b4f-8b2c-1f5a7fdf1602';
const session = {
  id: sessionId,
  state: 'ready',
  configRevision: 1,
  lastSequence: 2,
  createdAt: new Date(0).toISOString(),
  updatedAt: new Date(0).toISOString(),
};

const storedEvent = (sequence: number): SessionEvent => ({
  sessionId,
  promptId,
  sequence,
  type: 'reasoning.delta',
  event: { type: 'reasoning.delta', delta: `part-${sequence}` },
  createdAt: new Date(sequence * 1000).toISOString(),
});

const socketHost = async (store: unknown) => {
  const server = createServer();
  const io = new SocketServer(server);
  const publisher = createSessionsSocket(io, store as never);
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  const address = server.address();
  assert.ok(address !== null && typeof address === 'object');
  return {
    url: `http://127.0.0.1:${address.port}/sessions`,
    publisher,
    close: () => new Promise<void>((resolve) => io.close(() => resolve())),
  };
};

const next = <Value = unknown>(socket: Socket, event: string): Promise<Value> =>
  new Promise((resolve) => socket.once(event, resolve));
