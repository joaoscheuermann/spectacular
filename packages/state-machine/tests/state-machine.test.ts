import assert from 'node:assert/strict';
import test from 'node:test';

import {
  StateMachineError,
  createStateMachine,
  type StateMachineErrorCode,
  type StateMachineHandler,
  type StateMachineHandlers,
} from '../src/index.js';

type State = 'start' | 'middle' | 'done';
type Context = {
  readonly runId: string;
};
type Artifacts = {
  readonly start: {
    readonly input: string;
  };
  readonly middle: {
    readonly count: number;
  };
  readonly done: {
    readonly output: string;
  };
};
type DomainFailure = {
  readonly code: 'cancelled';
};

test('checks exhaustive handlers and state-specific artifacts at compile time', () => {
  const definition = createStateMachine<
    State,
    Context,
    Artifacts,
    string,
    DomainFailure
  >({
    start: (artifacts, { state, transition }) => {
      const current: 'start' = state;
      const input: string = artifacts.input;
      void current;
      void input;

      // @ts-expect-error start artifacts do not expose middle's count.
      const count: number = artifacts.count;
      void count;

      // @ts-expect-error middle transitions require middle artifacts.
      return transition('middle', { output: 'wrong' });
    },
    middle: (artifacts, { transition }) =>
      transition('done', { output: String(artifacts.count) }),
    done: (artifacts, { finish, fail }) => {
      // @ts-expect-error fail requires the configured domain failure.
      void fail();

      return artifacts.output === ''
        ? fail({ code: 'cancelled' })
        : finish(artifacts.output);
    },
  });

  const assertTypes = () => {
    void definition.run({
      context: { runId: 'typed' },
      state: 'start',
      artifacts: { input: 'ok' },
    });

    void definition.run({
      context: { runId: 'typed' },
      state: 'start',
      // @ts-expect-error initial start state requires start artifacts.
      artifacts: { count: 1 },
    });

    // @ts-expect-error every state requires a handler.
    createStateMachine<State, Context, Artifacts, string, DomainFailure>({
      start: (_artifacts, { finish }) => finish('start'),
      middle: (_artifacts, { finish }) => finish('middle'),
    });

    const supported: StateMachineErrorCode = 'handler_failed';
    void supported;

    // @ts-expect-error lifecycle errors were removed from reusable definitions.
    const removed: StateMachineErrorCode = 'concurrent_dispatch';
    void removed;
  };

  assert.equal(typeof assertTypes, 'function');
});

test('runs an asynchronous handler chain to completion', async () => {
  const definition = createStateMachine<
    State,
    Context,
    Artifacts,
    string,
    DomainFailure
  >({
    start: async (artifacts, { transition }) => {
      await Promise.resolve();

      return transition('middle', { count: artifacts.input.length });
    },
    middle: (artifacts, { transition }) =>
      transition('done', { output: String(artifacts.count) }),
    done: (artifacts, { finish }) => finish(artifacts.output),
  });
  const context = { runId: 'chain' };

  const result = await definition.run({
    context,
    state: 'start',
    artifacts: { input: 'doric' },
  });

  assert.deepEqual(result, {
    status: 'finished',
    value: '5',
    state: 'done',
    context,
  });
});

test('returns the exact domain failure passed to fail', async () => {
  const failure: DomainFailure = { code: 'cancelled' };
  const definition = createStateMachine<
    State,
    Context,
    Artifacts,
    string,
    DomainFailure
  >({
    start: (_artifacts, { fail }) => fail(failure),
    middle: (_artifacts, { finish }) => finish('middle'),
    done: (artifacts, { finish }) => finish(artifacts.output),
  });
  const context = { runId: 'failed' };

  const result = await definition.run({
    context,
    state: 'start',
    artifacts: { input: 'stop' },
  });

  assert.equal(result.status, 'failed');

  if (result.status === 'failed') {
    assert.equal(result.error, failure);
    assert.equal(result.state, 'start');
    assert.equal(result.context, context);
  }
});

test('preserves a thrown handler value as the engine error cause', async () => {
  const thrown = { reason: 'boom' };
  const definition = createStateMachine<
    State,
    Context,
    Artifacts,
    void,
    DomainFailure
  >({
    start: () => {
      throw thrown;
    },
    middle: (_artifacts, { finish }) => finish(),
    done: (_artifacts, { finish }) => finish(),
  });

  const result = await definition.run({
    context: { runId: 'thrown' },
    state: 'start',
    artifacts: { input: 'bad' },
  });

  assert.equal(result.status, 'error');

  if (result.status === 'error') {
    assert.ok(result.error instanceof StateMachineError);
    assert.equal(result.error.data.code, 'handler_failed');
    assert.equal(result.error.cause, thrown);
  }
});

test('reuses one definition for independent sequential runs', async () => {
  const definition = createStateMachine<
    State,
    Context,
    Artifacts,
    string,
    DomainFailure
  >({
    start: (artifacts, { context, finish }) =>
      finish(`${context.runId}:${artifacts.input}`),
    middle: (_artifacts, { finish }) => finish('middle'),
    done: (artifacts, { finish }) => finish(artifacts.output),
  });

  const first = await definition.run({
    context: { runId: 'first' },
    state: 'start',
    artifacts: { input: 'one' },
  });
  const second = await definition.run({
    context: { runId: 'second' },
    state: 'start',
    artifacts: { input: 'two' },
  });

  assert.equal(
    first.status === 'finished' ? first.value : undefined,
    'first:one',
  );
  assert.equal(
    second.status === 'finished' ? second.value : undefined,
    'second:two',
  );
});

test('isolates concurrent runs of one definition', async () => {
  type ConcurrentArtifacts = {
    readonly start: {
      readonly input: string;
      readonly gate: Promise<void>;
    };
  };

  const definition = createStateMachine<
    'start',
    Context,
    ConcurrentArtifacts,
    string,
    never
  >({
    start: async (artifacts, { context, finish }) => {
      await artifacts.gate;

      return finish(`${context.runId}:${artifacts.input}`);
    },
  });
  let releaseFirst: () => void = () => undefined;
  let releaseSecond: () => void = () => undefined;
  const firstGate = new Promise<void>((resolve) => {
    releaseFirst = resolve;
  });
  const secondGate = new Promise<void>((resolve) => {
    releaseSecond = resolve;
  });

  const first = definition.run({
    context: { runId: 'first' },
    state: 'start',
    artifacts: { input: 'one', gate: firstGate },
  });
  const second = definition.run({
    context: { runId: 'second' },
    state: 'start',
    artifacts: { input: 'two', gate: secondGate },
  });

  releaseSecond();
  const secondResult = await second;
  releaseFirst();
  const firstResult = await first;

  assert.equal(
    firstResult.status === 'finished' ? firstResult.value : undefined,
    'first:one',
  );
  assert.equal(
    secondResult.status === 'finished' ? secondResult.value : undefined,
    'second:two',
  );
});

test('returns an engine error when a handler returns an invalid action', async () => {
  const definition = createStateMachine<
    State,
    Context,
    Artifacts,
    void,
    DomainFailure
  >({
    start: (() => undefined) as unknown as StateMachineHandler<
      State,
      'start',
      Context,
      Artifacts,
      void,
      DomainFailure
    >,
    middle: (_artifacts, { finish }) => finish(),
    done: (_artifacts, { finish }) => finish(),
  });

  const result = await definition.run({
    context: { runId: 'invalid' },
    state: 'start',
    artifacts: { input: 'bad' },
  });

  assert.equal(result.status, 'error');

  if (result.status === 'error') {
    assert.equal(result.error.data.code, 'invalid_handler_return');
    assert.equal(result.state, 'start');
  }
});

test('defensively returns an engine error when a handler is missing', async () => {
  const handlers = {} as StateMachineHandlers<
    State,
    Context,
    Artifacts,
    void,
    DomainFailure
  >;
  const definition = createStateMachine<
    State,
    Context,
    Artifacts,
    void,
    DomainFailure
  >(handlers);

  const result = await definition.run({
    context: { runId: 'missing' },
    state: 'start',
    artifacts: { input: 'none' },
  });

  assert.equal(result.status, 'error');

  if (result.status === 'error') {
    assert.equal(result.error.data.code, 'missing_handler');
    assert.equal(result.state, 'start');
  }
});
