import type { Message, TaskState } from '@a2a-js/sdk';
import type { AgentExecutionEvent } from '@a2a-js/sdk/server';

export type RuntimeSessionSummary = {
  readonly contextId: string;
  readonly state: TaskState;
  readonly prompt: string;
  readonly latestTaskId?: string;
};

export type RuntimeSession = RuntimeSessionSummary & {
  readonly events: readonly AgentExecutionEvent[];
};

export type RuntimeReplayCompleteEvent = {
  readonly kind: 'doric/replay-complete';
  readonly contextId: string;
};

export type RuntimeConnectEvent = AgentExecutionEvent | RuntimeReplayCompleteEvent;

export type RuntimeStore = {
  observeMessage(message: Message): void;
  recordEvent(event: AgentExecutionEvent): void;
  list(): readonly RuntimeSessionSummary[];
  get(contextId: string): RuntimeSession | undefined;
  has(contextId: string): boolean;
  latestTaskId(contextId: string): string | undefined;
  delete(contextId: string): boolean;
  connect(contextId: string): AsyncGenerator<RuntimeConnectEvent, void>;
};

type RuntimeEventRecord = {
  readonly sequence: number;
  readonly event: AgentExecutionEvent;
};

type MutableRuntimeSession = {
  readonly contextId: string;
  prompt: string;
  state: TaskState;
  latestTaskId?: string;
  readonly events: RuntimeEventRecord[];
  readonly subscribers: Set<RuntimeSubscriber>;
};

type RuntimeSubscriber = {
  push(record: RuntimeEventRecord): void;
  close(): void;
  next(): Promise<RuntimeEventRecord | undefined>;
};

const TERMINAL_STATES = new Set<TaskState>([
  'completed',
  'canceled',
  'failed',
  'rejected',
]);

/** Creates Doric's process-local runtime session/event replay store. */
export const createRuntimeStore = (): RuntimeStore => {
  const sessions = new Map<string, MutableRuntimeSession>();
  const ignoredContextIds = new Set<string>();
  let sequence = 0;

  const ensure = (contextId: string): MutableRuntimeSession => {
    const existing = sessions.get(contextId);

    if (existing !== undefined) {
      return existing;
    }

    const created = {
      contextId,
      prompt: '',
      state: 'submitted',
      events: [],
      subscribers: new Set<RuntimeSubscriber>(),
    } satisfies MutableRuntimeSession;

    sessions.set(contextId, created);

    return created;
  };

  return {
    observeMessage(message) {
      if (message.contextId === undefined) {
        return;
      }

      ignoredContextIds.delete(message.contextId);

      const session = ensure(message.contextId);
      const prompt = messageText(message);

      if (session.prompt === '' && prompt !== '') {
        session.prompt = prompt;
      }

      if (message.taskId !== undefined) {
        session.latestTaskId = message.taskId;
      }
    },

    recordEvent(event) {
      const contextId = eventContextId(event);

      if (contextId === undefined || ignoredContextIds.has(contextId)) {
        return;
      }

      const session = ensure(contextId);
      const taskId = eventTaskId(event);
      const state = eventState(event);

      if (taskId !== undefined) {
        session.latestTaskId = taskId;
      }

      if (state !== undefined) {
        session.state = state;
      }

      const record = { sequence: (sequence += 1), event };
      session.events.push(record);

      for (const subscriber of session.subscribers) {
        subscriber.push(record);
      }

      if (isTerminal(session.state)) {
        for (const subscriber of session.subscribers) {
          subscriber.close();
        }
      }
    },

    list() {
      return [...sessions.values()].map(summary);
    },

    get(contextId) {
      const session = sessions.get(contextId);

      if (session === undefined) {
        return undefined;
      }

      return {
        ...summary(session),
        events: session.events.map((record) => record.event),
      };
    },

    has(contextId) {
      return sessions.has(contextId);
    },

    latestTaskId(contextId) {
      return sessions.get(contextId)?.latestTaskId;
    },

    delete(contextId) {
      ignoredContextIds.add(contextId);

      const session = sessions.get(contextId);

      if (session === undefined) {
        return false;
      }

      for (const subscriber of session.subscribers) {
        subscriber.close();
      }

      return sessions.delete(contextId);
    },

    async *connect(contextId) {
      const session = sessions.get(contextId);

      if (session === undefined) {
        return;
      }

      const subscriber = createSubscriber();
      session.subscribers.add(subscriber);

      try {
        let lastSequence = 0;

        for (const record of session.events) {
          lastSequence = record.sequence;
          yield record.event;
        }

        const replayEndedTerminal = isTerminal(session.state);

        yield { kind: 'doric/replay-complete', contextId };

        if (replayEndedTerminal) {
          return;
        }

        while (true) {
          const record = await subscriber.next();

          if (record === undefined) {
            return;
          }

          if (record.sequence <= lastSequence) {
            continue;
          }

          lastSequence = record.sequence;
          yield record.event;

          if (isTerminal(sessions.get(contextId)?.state ?? 'unknown')) {
            return;
          }
        }
      } finally {
        session.subscribers.delete(subscriber);
      }
    },
  };
};

const createSubscriber = (): RuntimeSubscriber => {
  const queue: RuntimeEventRecord[] = [];
  let closed = false;
  let wake: (() => void) | undefined;

  return {
    push(record) {
      if (closed) {
        return;
      }

      queue.push(record);
      wake?.();
      wake = undefined;
    },

    close() {
      closed = true;
      wake?.();
      wake = undefined;
    },

    async next() {
      while (!closed && queue.length === 0) {
        await new Promise<void>((resolve) => {
          wake = resolve;
        });
      }

      return queue.shift();
    },
  };
};

const summary = (session: MutableRuntimeSession): RuntimeSessionSummary => ({
  contextId: session.contextId,
  state: session.state,
  prompt: session.prompt,
  ...(session.latestTaskId === undefined
    ? {}
    : { latestTaskId: session.latestTaskId }),
});

const messageText = (message: Message): string =>
  message.parts
    .filter((part) => part.kind === 'text')
    .map((part) => part.text)
    .join('\n')
    .trim();

const eventContextId = (event: AgentExecutionEvent): string | undefined => {
  if (event.kind === 'task') {
    return event.contextId;
  }

  return event.contextId;
};

const eventTaskId = (event: AgentExecutionEvent): string | undefined => {
  if (event.kind === 'task') {
    return event.id;
  }

  return event.taskId;
};

const eventState = (event: AgentExecutionEvent): TaskState | undefined => {
  if (event.kind === 'task' || event.kind === 'status-update') {
    return event.status.state;
  }

  return undefined;
};

const isTerminal = (state: TaskState): boolean => TERMINAL_STATES.has(state);
