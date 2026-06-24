import assert from 'node:assert/strict';
import test from 'node:test';

import type { AgentExecutionEvent } from '@a2a-js/sdk/server';

import { writeEvent, writeSessionList } from '../src/lib/render.js';
import { createTextRedactor } from '../src/lib/redact.js';

type TaskState = Extract<
  AgentExecutionEvent,
  { readonly kind: 'status-update' }
>['status']['state'];

test('prints session rows as context, state, and prompt', () => {
  const output: string[] = [];

  writeSessionList(
    {
      stdout: {
        write(text) {
          output.push(text);
        },
      },
    },
    [
      {
        contextId: 'context-1',
        state: 'input-required',
        prompt: 'Build\nCLI',
      },
    ],
  );

  assert.deepEqual(output, ['context-1 input-required Build CLI\n']);
});

test('writes redacted pino-pretty event lines with structured fields', async () => {
  const output: string[] = [];
  const redactText = createTextRedactor({
    CODEX_AUTHORIZATION: 'codex-token',
  });

  writeEvent(
    {
      stdout: {
        write(text) {
          output.push(text);
        },
      },
      stderr: {
        write(text) {
          output.push(text);
        },
      },
    },
    completedEvent('Use codex-token'),
    redactText,
  );
  await settle();

  const rendered = output.join('');
  const plain = stripAnsi(rendered);

  assert.match(rendered, /\u001b\[[0-9;]*m/u);
  assertTimestamp(plain);
  assert.match(plain, /SUCCESS/u);
  assert.match(plain, /Use \[redacted\]/u);
  assert.doesNotMatch(plain, /codex-token/u);
  assert.match(plain, /eventKind/u);
  assert.match(plain, /status-update/u);
  assert.match(plain, /context-1/u);
  assert.match(plain, /task-1/u);
  assert.match(plain, /completed/u);
  assert.match(plain, /final/u);
});

test('writes structured event lines for data-only events', async () => {
  const output: string[] = [];

  writeEvent(
    {
      stdout: {
        write(text) {
          output.push(text);
        },
      },
      stderr: {
        write(text) {
          output.push(text);
        },
      },
    },
    dataOnlyEvent(),
    (text) => text,
  );
  await settle();

  const rendered = stripAnsi(output.join(''));

  assertTimestamp(rendered);
  assert.match(rendered, /WARN/u);
  assert.match(rendered, /status-update/u);
  assert.match(rendered, /context-1/u);
  assert.match(rendered, /task-1/u);
  assert.match(rendered, /input-required/u);
  assert.match(rendered, /final/u);
});

test('writes semantic level labels for event states', async () => {
  const cases = [
    {
      event: statusEvent('working', 'Work started', false),
      level: 'INFO',
      text: 'Work started',
    },
    {
      event: statusEvent('input-required', 'Choose an option', true),
      level: 'WARN',
      text: 'Choose an option',
    },
    {
      event: statusEvent('failed', 'Task failed', true),
      level: 'ERROR',
      text: 'Task failed',
    },
    {
      event: completedEvent('Task completed'),
      level: 'SUCCESS',
      text: 'Task completed',
    },
  ];

  for (const entry of cases) {
    const rendered = stripAnsi(await renderEvent(entry.event));

    assert.match(rendered, new RegExp(`\\b${entry.level}\\b`, 'u'));
    assert.match(rendered, new RegExp(entry.text, 'u'));
  }
});

const completedEvent = (text: string): AgentExecutionEvent =>
  statusEvent('completed', text, true);

const statusEvent = (
  state: TaskState,
  text: string,
  final: boolean,
): AgentExecutionEvent => ({
  kind: 'status-update',
  taskId: 'task-1',
  contextId: 'context-1',
  final,
  status: {
    state,
    message: {
      kind: 'message',
      messageId: 'message-1',
      role: 'agent',
      taskId: 'task-1',
      contextId: 'context-1',
      parts: [{ kind: 'text', text }],
    },
  },
});

const dataOnlyEvent = (): AgentExecutionEvent => ({
  kind: 'status-update',
  taskId: 'task-1',
  contextId: 'context-1',
  final: true,
  status: {
    state: 'input-required',
    message: {
      kind: 'message',
      messageId: 'message-1',
      role: 'agent',
      taskId: 'task-1',
      contextId: 'context-1',
      parts: [
        {
          kind: 'data',
          data: {
            kind: 'prompt-open-questions',
            questions: [],
          },
        },
      ],
    },
  },
});

const settle = (): Promise<void> =>
  new Promise((resolve) => {
    setImmediate(resolve);
  });

const renderEvent = async (event: AgentExecutionEvent): Promise<string> => {
  const output: string[] = [];

  writeEvent(
    {
      stdout: {
        write(text) {
          output.push(text);
        },
      },
      stderr: {
        write(text) {
          output.push(text);
        },
      },
    },
    event,
    (text) => text,
  );
  await settle();

  return output.join('');
};

const stripAnsi = (text: string): string =>
  text.replaceAll(/\u001b\[[0-9;]*m/gu, '');

const assertTimestamp = (text: string): void => {
  assert.match(text, /\[\d{2}:\d{2}:\d{2}\.\d{3}\]/u);
};
