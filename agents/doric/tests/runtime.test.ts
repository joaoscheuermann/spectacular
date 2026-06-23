import assert from 'node:assert/strict';
import test from 'node:test';

import { createRuntimeStore } from '../src/lib/runtime/store.js';
import {
  promptCompletedMessage,
  promptInputRequiredMessage,
  taskCreatedMessage,
} from '../src/lib/messages/index.js';
import { createTextPart, createUserMessage } from './fakes.js';

test('lists runtime sessions with prompt, latest task, and visible state', () => {
  const runtime = createRuntimeStore();

  runtime.observeMessage(
    createUserMessage({
      contextId: 'context-1',
      parts: [createTextPart('Build the CLI')],
    }),
  );
  runtime.recordEvent(taskCreatedMessage('task-1', 'context-1'));
  runtime.recordEvent(promptCompletedMessage('task-1', 'context-1'));

  assert.deepEqual(runtime.list(), [
    {
      contextId: 'context-1',
      state: 'completed',
      prompt: 'Build the CLI',
      latestTaskId: 'task-1',
    },
  ]);
});

test('replays history and follows live events until the session is terminal', async () => {
  const runtime = createRuntimeStore();

  runtime.observeMessage(
    createUserMessage({
      contextId: 'context-1',
      parts: [createTextPart('Build the CLI')],
    }),
  );
  runtime.recordEvent(taskCreatedMessage('task-1', 'context-1'));

  const stream = runtime.connect('context-1');

  const replay = await stream.next();
  assert.equal(replay.value?.kind, 'task');

  const replayBoundary = await stream.next();
  assert.deepEqual(replayBoundary.value, {
    kind: 'doric/replay-complete',
    contextId: 'context-1',
  });

  runtime.recordEvent(promptCompletedMessage('task-1', 'context-1'));

  const live = await stream.next();
  assert.equal(live.value?.kind, 'status-update');
  assert.equal(live.value?.kind === 'status-update' ? live.value.final : false, true);

  assert.deepEqual(await stream.next(), { value: undefined, done: true });
});

test('keeps connect streams open when input is required', async () => {
  const runtime = createRuntimeStore();

  runtime.observeMessage(
    createUserMessage({
      contextId: 'context-1',
      parts: [createTextPart('Build the CLI')],
    }),
  );
  runtime.recordEvent(taskCreatedMessage('task-1', 'context-1'));

  const stream = runtime.connect('context-1');

  assert.equal((await stream.next()).value?.kind, 'task');
  assert.deepEqual((await stream.next()).value, {
    kind: 'doric/replay-complete',
    contextId: 'context-1',
  });

  runtime.recordEvent(
    promptInputRequiredMessage('task-1', 'context-1', [
      {
        question: 'Which branch?',
        recommendation: 'Use the configured branch.',
        options: ['main', 'develop', 'feature/doric-cli'],
      },
    ]),
  );

  assert.equal((await stream.next()).value?.kind, 'status-update');

  const pending = Promise.race([
    stream.next().then(() => 'closed'),
    new Promise<'open'>((resolve) => setTimeout(() => resolve('open'), 20)),
  ]);

  assert.equal(await pending, 'open');
  runtime.delete('context-1');
  assert.deepEqual(await stream.next(), { value: undefined, done: true });
});
