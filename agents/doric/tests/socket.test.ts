import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import test from 'node:test';

import { io as connect, type Socket } from 'socket.io-client';
import { Server as SocketServer } from 'socket.io';

import { createMosaicSocket } from '../src/lib/socket.js';
import type { SessionEvent } from '../src/lib/sessions.js';

test('replays only missing events and resumes live delivery after reconnect', async () => {
  const events = [storedEvent(1), storedEvent(2)];
  const store = {
    find: async () => ({ session: session, snapshot: {} }),
    eventsAfter: async (_id: string, sequence: number) =>
      events.filter((event) => event.sequence > sequence),
  };
  const server = createServer();
  const io = new SocketServer(server);
  const publisher = createMosaicSocket(io, store as never);
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  const address = server.address();
  assert.ok(address !== null && typeof address === 'object');
  const url = `http://127.0.0.1:${address.port}/mosaic`;

  const first = await client(url);
  const firstSnapshot = next<{ events: unknown[] }>(first, 'session:snapshot');
  first.emit('session:subscribe', { sessionId, afterSequence: 1 });
  assert.deepEqual((await firstSnapshot).events, [events[1]!.event]);

  const live = next<{ sessionId: string; event: unknown }>(
    first,
    'mosaic:event',
  );
  const third = storedEvent(3);
  events.push(third);
  publisher.event(third);
  assert.deepEqual(await live, { sessionId, event: third.event });
  first.close();

  events.push(storedEvent(4));
  const second = await client(url);
  const secondSnapshot = next<{ events: unknown[] }>(
    second,
    'session:snapshot',
  );
  second.emit('session:subscribe', { sessionId, afterSequence: 3 });
  assert.deepEqual((await secondSnapshot).events, [events[3]!.event]);
  second.close();
  await new Promise<void>((resolve) => io.close(() => resolve()));
});

const sessionId = '018f47d2-e3b1-7b4f-8b2c-1f5a7fdf1601';
const session = {
  id: sessionId,
  prompt: 'request',
  state: 'running',
  configRevision: 1,
  result: null,
  lastSequence: 2,
  createdAt: new Date(0).toISOString(),
  updatedAt: new Date(0).toISOString(),
};

const storedEvent = (sequence: number): SessionEvent => ({
  sessionId,
  sequence,
  type: 'stage.started',
  event: {
    schemaVersion: 2,
    runId: sessionId,
    sequence,
    type: 'stage.started',
    stage: 'plan',
  },
  createdAt: new Date(sequence * 1000).toISOString(),
});

const client = async (url: string): Promise<Socket> => {
  const socket = connect(url, { transports: ['websocket'] });
  await next(socket, 'connect');
  return socket;
};

const next = <Value = unknown>(socket: Socket, event: string): Promise<Value> =>
  new Promise((resolve) => socket.once(event, resolve));
