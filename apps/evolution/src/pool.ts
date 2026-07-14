type Task<Value> = () => Promise<Value>;

type QueuedTask = {
  readonly run: Task<unknown>;
  readonly resolve: (value: unknown) => void;
  readonly reject: (reason?: unknown) => void;
};

type FailureListener = (reason: unknown) => void;

type FailureState = {
  failed: boolean;
  reason: unknown;
  readonly listeners: Set<FailureListener>;
};

export type Failure = {
  readonly failed: () => boolean;
  readonly reason: () => unknown;
  readonly fail: (reason: unknown) => unknown;
  readonly subscribe: (listener: (reason: unknown) => void) => void;
};

export type Pool = {
  readonly run: <Value>(task: Task<Value>) => Promise<Value>;
  readonly idle: () => Promise<void>;
};

type PoolState = {
  readonly limit: number;
  readonly failure: Failure;
  readonly queue: QueuedTask[];
  readonly idleWaiters: Set<() => void>;
  active: number;
};

const fail = (state: FailureState, error: unknown): unknown => {
  if (state.failed) return state.reason;
  state.failed = true;
  state.reason = error;
  for (const listener of state.listeners) listener(state.reason);
  return state.reason;
};

const subscribe = (state: FailureState, listener: FailureListener): void => {
  if (state.failed) {
    listener(state.reason);
    return;
  }
  state.listeners.add(listener);
};

/** Captures and broadcasts the first failure, including an undefined rejection. */
export const createFailure = (): Failure => {
  const state: FailureState = {
    failed: false,
    reason: undefined,
    listeners: new Set(),
  };
  return {
    failed: () => state.failed,
    reason: () => state.reason,
    fail: (error) => fail(state, error),
    subscribe: (listener) => subscribe(state, listener),
  };
};

const settleIdle = (state: PoolState): void => {
  if (state.active !== 0 || state.queue.length !== 0) return;
  for (const resolve of state.idleWaiters) resolve();
  state.idleWaiters.clear();
};

const rejectQueued = (state: PoolState, reason: unknown): void => {
  const queued = state.queue.splice(0);
  for (const task of queued) task.reject(reason);
  settleIdle(state);
};

const start = (state: PoolState, task: QueuedTask): void => {
  state.active += 1;
  Promise.resolve()
    .then(() => {
      if (state.failure.failed()) throw state.failure.reason();
      return task.run();
    })
    .then(task.resolve, (error: unknown) => {
      task.reject(state.failure.fail(error));
    })
    .finally(() => {
      state.active -= 1;
      drain(state);
      settleIdle(state);
    });
};

const drain = (state: PoolState): void => {
  while (
    !state.failure.failed() &&
    state.active < state.limit &&
    state.queue.length > 0
  ) {
    const task = state.queue.shift();
    if (task !== undefined) start(state, task);
  }
};

const enqueue = <Value>(state: PoolState, task: Task<Value>): Promise<Value> => {
  if (state.failure.failed()) return Promise.reject(state.failure.reason());
  return new Promise<Value>((resolve, reject) => {
    state.queue.push({
      run: task,
      resolve: (value) => resolve(value as Value),
      reject,
    });
    drain(state);
  });
};

const idle = (state: PoolState): Promise<void> => {
  if (state.active === 0 && state.queue.length === 0) return Promise.resolve();
  return new Promise((resolve) => state.idleWaiters.add(resolve));
};

/** Runs queued promise tasks through a sliding limit shared by every caller. */
export const createPool = (limit: number, failure: Failure): Pool => {
  const state: PoolState = {
    limit,
    failure,
    queue: [],
    idleWaiters: new Set(),
    active: 0,
  };

  failure.subscribe((reason) => rejectQueued(state, reason));
  return {
    run: (task) => enqueue(state, task),
    idle: () => idle(state),
  };
};
