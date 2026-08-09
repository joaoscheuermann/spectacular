import type { Server, Socket } from 'socket.io';
import { z } from 'zod';

import type { SessionPublisher } from './session-publisher.js';
import type { Session, SessionEvent, SessionStore } from './sessions.js';

const subscriptionInput = z
  .object({
    sessionId: z.uuid(),
    afterSequence: z.number().int().safe().nonnegative().optional(),
  })
  .strict();
const unsubscribeInput = z.object({ sessionId: z.uuid() }).strict();

type Subscription = {
  ready: boolean;
  lastSequence: number;
  readonly buffered: SessionEvent[];
  update?: Session;
  deleted: boolean;
};

/** Attaches the durable Mosaic replay namespace and returns its publisher. */
export const createMosaicSocket = (
  io: Server,
  store: SessionStore,
): SessionPublisher => {
  const namespace = io.of('/mosaic');
  const subscriptions = new Map<string, Map<Socket, Subscription>>();

  namespace.on('connection', (socket) => {
    socket.on('session:subscribe', async (input: unknown) => {
      const parsed = subscriptionInput.safeParse(input);
      if (!parsed.success) return;
      const { sessionId, afterSequence = 0 } = parsed.data;
      const subscription = subscribe(
        subscriptions,
        sessionId,
        socket,
        afterSequence,
      );
      const [record, events] = await Promise.all([
        store.find(sessionId),
        store.eventsAfter(sessionId, afterSequence),
      ]);
      socket.emit('session:snapshot', {
        sessionId,
        session: record?.session ?? null,
        events: events.map(({ event }) => event),
      });
      subscription.lastSequence = Math.max(
        afterSequence,
        events.at(-1)?.sequence ?? afterSequence,
      );
      subscription.ready = true;
      flush(subscriptions, socket, sessionId, subscription);
    });

    socket.on('session:unsubscribe', (input: unknown) => {
      const parsed = unsubscribeInput.safeParse(input);
      if (parsed.success)
        unsubscribe(subscriptions, parsed.data.sessionId, socket);
    });
    socket.on('disconnect', () => {
      subscriptions.forEach((_sockets, sessionId) =>
        unsubscribe(subscriptions, sessionId, socket),
      );
    });
  });

  return {
    event(value) {
      subscriptions.get(value.sessionId)?.forEach((subscription, socket) => {
        if (!subscription.ready) {
          subscription.buffered.push(value);
          return;
        }
        emitEvent(socket, value.sessionId, subscription, value);
      });
    },
    updated(value) {
      subscriptions.get(value.id)?.forEach((subscription, socket) => {
        if (!subscription.ready) {
          subscription.update = value;
          return;
        }
        socket.emit('session:updated', value);
      });
    },
    deleted(id) {
      subscriptions.get(id)?.forEach((subscription, socket) => {
        if (!subscription.ready) {
          subscription.deleted = true;
          return;
        }
        socket.emit('session:deleted', { sessionId: id });
        unsubscribe(subscriptions, id, socket);
      });
    },
  };
};

const subscribe = (
  subscriptions: Map<string, Map<Socket, Subscription>>,
  sessionId: string,
  socket: Socket,
  afterSequence: number,
): Subscription => {
  const sockets =
    subscriptions.get(sessionId) ?? new Map<Socket, Subscription>();
  const subscription = {
    ready: false,
    lastSequence: afterSequence,
    buffered: [],
    deleted: false,
  };
  sockets.set(socket, subscription);
  subscriptions.set(sessionId, sockets);
  return subscription;
};

const unsubscribe = (
  subscriptions: Map<string, Map<Socket, Subscription>>,
  sessionId: string,
  socket: Socket,
) => {
  const sockets = subscriptions.get(sessionId);
  sockets?.delete(socket);
  if (sockets?.size === 0) subscriptions.delete(sessionId);
};

const flush = (
  subscriptions: Map<string, Map<Socket, Subscription>>,
  socket: Socket,
  sessionId: string,
  subscription: Subscription,
) => {
  subscription.buffered
    .sort((left, right) => left.sequence - right.sequence)
    .forEach((event) => emitEvent(socket, sessionId, subscription, event));
  subscription.buffered.length = 0;
  if (subscription.update !== undefined) {
    socket.emit('session:updated', subscription.update);
    subscription.update = undefined;
  }
  if (subscription.deleted) {
    socket.emit('session:deleted', { sessionId });
    unsubscribe(subscriptions, sessionId, socket);
  }
};

const emitEvent = (
  socket: Socket,
  sessionId: string,
  subscription: Subscription,
  event: SessionEvent,
) => {
  if (event.sequence <= subscription.lastSequence) return;
  socket.emit('mosaic:event', { sessionId, event: event.event });
  subscription.lastSequence = event.sequence;
};
