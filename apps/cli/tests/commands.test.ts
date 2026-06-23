import assert from 'node:assert/strict';
import { resolve } from 'node:path';
import test from 'node:test';

import type { AgentExecutionEvent } from '@a2a-js/sdk/server';

import {
  runConnect,
  runList,
  type CommandDependencies,
} from '../src/lib/commands.js';

type JsonRpcRequest = {
  readonly id: number;
  readonly method: string;
  readonly params: unknown;
};

type JsonRpcHandler = (request: JsonRpcRequest) => unknown;

const EMPTY_ENV_ROOT = resolve('.missing-cli-test-env');

test('prints sessions from the custom list response object', async () => {
  const output: string[] = [];
  const fetch = createFetch((request) => {
    assert.equal(request.method, 'doric/sessions/list');

    return {
      sessions: [
        {
          contextId: 'context-1',
          state: 'completed',
          prompt: 'Build\nCLI',
        },
      ],
    };
  });

  await runList({
    ...dependencies(fetch),
    io: captureIo(output),
  });

  assert.deepEqual(output, ['context-1 completed Build CLI\n']);
});

test('does not answer input-required events replayed before connect replay completes', async () => {
  const output: string[] = [];
  const methods: string[] = [];
  const fetch = createFetch((request) => {
    methods.push(request.method);

    if (request.method === 'doric/sessions/connect') {
      return connectStream(request.id, [
        inputRequiredEvent(),
        completedEvent(),
        { kind: 'doric/replay-complete', contextId: 'context-1' },
      ]);
    }

    return {};
  });

  await runConnect('context-1', {
    ...dependencies(fetch),
    io: captureIo(output),
    prompt: async () => {
      throw new Error('Prompt should not be called for replayed input.');
    },
  });

  assert.deepEqual(methods, ['doric/sessions/connect']);
  assert.deepEqual(output, ['Prompt workflow completed.\n']);
});

const dependencies = (fetch: typeof globalThis.fetch): CommandDependencies => ({
  fetch,
  io: captureIo([]),
  prompt: async () => ({}),
  rootDir: EMPTY_ENV_ROOT,
  env: {
    AGENT_CARD_URL: 'http://agent-card.example',
    CODEX_AUTHORIZATION: 'codex-token',
    GITHUB_TOKEN: 'github-token',
  },
});

const captureIo = (output: string[]): CommandDependencies['io'] => ({
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
});

const createFetch =
  (handler: JsonRpcHandler): typeof globalThis.fetch =>
  async (input, init) => {
    const url = String(input);

    if (url === 'http://agent-card.example') {
      return Response.json({ url: 'http://rpc.example' });
    }

    assert.equal(url, 'http://rpc.example');
    const request = JSON.parse(String(init?.body ?? '')) as JsonRpcRequest;
    const result = handler(request);

    if (result instanceof Response) {
      return result;
    }

    return Response.json({
      jsonrpc: '2.0',
      id: request.id,
      result,
    });
  };

const connectStream = (
  id: number,
  results: readonly unknown[],
): Response =>
  new Response(
    results
      .map((result) =>
        `data: ${JSON.stringify({ jsonrpc: '2.0', id, result })}\n\n`,
      )
      .join(''),
    { headers: { 'Content-Type': 'text/event-stream' } },
  );

const inputRequiredEvent = (): AgentExecutionEvent => ({
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
            questions: [
              {
                id: 'question-1',
                question: 'Which branch?',
                options: [
                  { title: 'main', value: 'main' },
                  { title: 'develop', value: 'develop' },
                  { title: 'feature/doric-cli', value: 'feature/doric-cli' },
                ],
              },
            ],
          },
        },
      ],
    },
  },
});

const completedEvent = (): AgentExecutionEvent => ({
  kind: 'status-update',
  taskId: 'task-1',
  contextId: 'context-1',
  final: true,
  status: {
    state: 'completed',
    message: {
      kind: 'message',
      messageId: 'message-2',
      role: 'agent',
      taskId: 'task-1',
      contextId: 'context-1',
      parts: [{ kind: 'text', text: 'Prompt workflow completed.' }],
    },
  },
});
