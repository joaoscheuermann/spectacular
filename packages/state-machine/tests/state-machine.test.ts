import assert from 'node:assert/strict';
import test from 'node:test';

import {
  StateMachineErrorObject,
  createStateMachine,
  type StateMachineHandler,
} from '../src/index.js';

type State = 'start' | 'middle' | 'done' | 'recover';
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
  readonly recover: {
    readonly reason: string;
  };
};

test('checks state-specific artifacts at compile time', async () => {
  const machine = createStateMachine<State, Context, Artifacts>({
    runId: 'typed',
  });

  const assertTypes = () => {
    void machine.dispatch('start', { input: 'ok' });

    // @ts-expect-error start artifacts require input, not output.
    void machine.dispatch('start', { output: 'wrong' });

    machine.register('middle', (artifacts, { finish }) => {
      const count: number = artifacts.count;
      void count;

      // @ts-expect-error middle artifacts do not expose input.
      const input: string = artifacts.input;
      void input;

      return finish();
    });
  };

  assert.equal(typeof assertTypes, 'function');
});

test('runs async handler chains and reports transition finish and status state', async () => {
  const machine = createStateMachine<State, Context, Artifacts, string>({
    runId: 'chain',
  });
  const transitions: string[] = [];
  const finished: string[] = [];

  machine.on('transition', (from, to) => transitions.push(`${from}->${to}`));
  machine.on('finish', (result) => finished.push(result.value ?? ''));

  machine
    .register('start', async (artifacts, { dispatch }) =>
      dispatch('middle', { count: artifacts.input.length }),
    )
    .register('middle', (artifacts, { dispatch }) =>
      dispatch('done', { output: String(artifacts.count) }),
    )
    .register('done', (artifacts, { finish }) => finish(artifacts.output));

  assert.equal(machine.status(), 'idle');

  const result = await machine.dispatch('start', { input: 'doric' });

  assert.deepEqual(transitions, ['start->middle', 'middle->done']);
  assert.deepEqual(finished, ['5']);
  assert.deepEqual(result, {
    status: 'finished',
    value: '5',
    state: 'done',
    context: { runId: 'chain' },
  });
  assert.equal(machine.isDone(), true);
  assert.equal(machine.status(), 'finished');
  assert.deepEqual(machine.result(), result);
});

test('continues to a recovery state when an error listener returns a transition', async () => {
  const machine = createStateMachine<State, Context, Artifacts, string>({
    runId: 'recoverable',
  });
  const errors: string[] = [];
  const transitions: string[] = [];

  machine.on('transition', (from, to) => transitions.push(`${from}->${to}`));
  machine.on('error', (error, dispatch) => {
    errors.push(error.data.code);

    return dispatch('recover', { reason: error.data.code });
  });
  machine.register('start', () => {
    throw new Error('boom');
  });
  machine.register('recover', (artifacts, { finish }) =>
    finish(artifacts.reason),
  );

  const result = await machine.dispatch('start', { input: 'bad' });

  assert.deepEqual(errors, ['handler_failed']);
  assert.deepEqual(transitions, ['start->recover']);
  assert.deepEqual(result, {
    status: 'finished',
    value: 'handler_failed',
    state: 'recover',
    context: { runId: 'recoverable' },
  });
});

test('resolves an error result when handler errors are not recovered', async () => {
  const machine = createStateMachine<State, Context, Artifacts>({
    runId: 'unrecovered',
  });

  machine.register('start', () => {
    throw new Error('boom');
  });

  const result = await machine.dispatch('start', { input: 'bad' });

  assert.equal(result.status, 'error');
  assert.equal(result.state, 'start');
  assert.ok(result.error instanceof StateMachineErrorObject);
  assert.equal(result.error.data.code, 'handler_failed');
  assert.equal(machine.isDone(), true);
  assert.equal(machine.status(), 'error');
});

test('rejects duplicate state registrations with structured errors', () => {
  const machine = createStateMachine<State, Context, Artifacts>({
    runId: 'duplicate',
  });

  machine.register('start', (_artifacts, { finish }) => finish());

  assert.throws(
    () => machine.register('start', (_artifacts, { finish }) => finish()),
    (error: unknown) =>
      error instanceof StateMachineErrorObject &&
      error.data.code === 'duplicate_state_registration',
  );
});

test('resolves a structured error when a handler is missing', async () => {
  const machine = createStateMachine<State, Context, Artifacts>({
    runId: 'missing',
  });

  const result = await machine.dispatch('start', { input: 'none' });

  assert.equal(result.status, 'error');
  assert.equal(result.state, 'start');
  assert.equal(result.error.data.code, 'missing_handler');
});

test('resolves a structured error when a handler returns an invalid action', async () => {
  const machine = createStateMachine<State, Context, Artifacts>({
    runId: 'invalid',
  });

  machine.register(
    'start',
    (() => undefined) as unknown as StateMachineHandler<
      State,
      'start',
      Context,
      Artifacts,
      void
    >,
  );

  const result = await machine.dispatch('start', { input: 'bad' });

  assert.equal(result.status, 'error');
  assert.equal(result.state, 'start');
  assert.equal(result.error.data.code, 'invalid_handler_return');
});

test('rejects concurrent public dispatch with a structured lifecycle error', async () => {
  const machine = createStateMachine<State, Context, Artifacts>({
    runId: 'concurrent',
  });
  let release: (() => void) | undefined;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });

  machine.register('start', async (_artifacts, { finish }) => {
    await gate;

    return finish();
  });

  const first = machine.dispatch('start', { input: 'wait' });

  await assert.rejects(
    () => machine.dispatch('start', { input: 'again' }),
    (error: unknown) =>
      error instanceof StateMachineErrorObject &&
      error.data.code === 'concurrent_dispatch',
  );

  release?.();
  await first;
});

test('rejects dispatch after terminal completion with a structured lifecycle error', async () => {
  const machine = createStateMachine<State, Context, Artifacts>({
    runId: 'terminal',
  });

  machine.register('start', (_artifacts, { finish }) => finish());

  await machine.dispatch('start', { input: 'done' });

  await assert.rejects(
    () => machine.dispatch('start', { input: 'again' }),
    (error: unknown) =>
      error instanceof StateMachineErrorObject &&
      error.data.code === 'terminal_dispatch',
  );
});
