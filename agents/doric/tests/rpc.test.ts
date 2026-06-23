import assert from 'node:assert/strict';
import test from 'node:test';

import type { AgentCard, Task } from '@a2a-js/sdk';
import { A2AError, type A2ARequestHandler } from '@a2a-js/sdk/server';
import { createSessionStore } from 'session';

import { createDoricRpcHandler } from '../src/lib/rpc.js';
import { taskCreatedMessage } from '../src/lib/messages/index.js';
import { createRuntimeStore } from '../src/lib/runtime/store.js';
import type { DoricSessionContext } from '../src/lib/executor.js';

test('returns Doric runtime sessions from the custom list method', async () => {
  const runtime = createRuntimeStore();
  const handler = createDoricRpcHandler({
    requestHandler: createRequestHandler(),
    runtime,
    sessions: createSessionStore<DoricSessionContext>(),
    userBuilder: async () => unauthenticatedUser,
  });

  runtime.recordEvent(taskCreatedMessage('task-1', 'context-1'));

  const response = await handler.handle({
    jsonrpc: '2.0',
    id: 1,
    method: 'doric/sessions/list',
    params: {},
  });

  assert.deepEqual('result' in response ? response.result : undefined, {
    sessions: [
      {
        contextId: 'context-1',
        state: 'working',
        prompt: '',
      },
    ],
  });
});

test('streams replayed events from the custom connect method', async () => {
  const runtime = createRuntimeStore();
  const handler = createDoricRpcHandler({
    requestHandler: createRequestHandler(),
    runtime,
    sessions: createSessionStore<DoricSessionContext>(),
    userBuilder: async () => unauthenticatedUser,
  });

  runtime.recordEvent(taskCreatedMessage('task-1', 'context-1'));

  const response = await handler.handle({
    jsonrpc: '2.0',
    id: 1,
    method: 'doric/sessions/connect',
    params: { contextId: 'context-1' },
  });

  assert.ok(Symbol.asyncIterator in response);

  const stream = response as AsyncGenerator<{ readonly result?: unknown }>;
  const first = await stream.next();

  assert.equal(
    isTask(first.value?.result) ? first.value.result.id : undefined,
    'task-1',
  );

  const replayBoundary = await stream.next();

  assert.deepEqual(replayBoundary.value?.result, {
    kind: 'doric/replay-complete',
    contextId: 'context-1',
  });
});

test('kills known sessions by canceling the latest task and disposing the sandbox', async () => {
  const runtime = createRuntimeStore();
  const sessions = createSessionStore<DoricSessionContext>();
  const cancels: string[] = [];
  const sandbox = createDisposableSandbox();
  const handler = createDoricRpcHandler({
    requestHandler: createRequestHandler({
      cancelTask: async (params) => {
        cancels.push(params.id);

        return canceledTask(params.id, 'context-1');
      },
    }),
    runtime,
    sessions,
    userBuilder: async () => unauthenticatedUser,
  });

  runtime.recordEvent(taskCreatedMessage('task-1', 'context-1'));
  await sessions.getOrCreate('context-1', async () => ({
    config: {} as DoricSessionContext['config'],
    repo: { path: '/workspace/repo', commit: 'abc123' },
    sandbox,
  }));

  const response = await handler.handle({
    jsonrpc: '2.0',
    id: 1,
    method: 'doric/sessions/kill',
    params: { contextId: 'context-1' },
  });

  assert.deepEqual('result' in response ? response.result : undefined, {
    contextId: 'context-1',
    killed: true,
  });
  assert.deepEqual(cancels, ['task-1']);
  assert.equal(sandbox.disposed, true);
  assert.equal(runtime.has('context-1'), false);
  assert.equal(sessions.get('context-1'), undefined);
});

test('returns an A2A not-found error when killing an unknown session', async () => {
  const handler = createDoricRpcHandler({
    requestHandler: createRequestHandler(),
    runtime: createRuntimeStore(),
    sessions: createSessionStore<DoricSessionContext>(),
    userBuilder: async () => unauthenticatedUser,
  });

  const response = await handler.handle({
    jsonrpc: '2.0',
    id: 1,
    method: 'doric/sessions/kill',
    params: { contextId: 'missing' },
  });

  assert.deepEqual('error' in response ? response.error : undefined, {
    code: -32001,
    message: 'Task not found: missing',
  });
});

const createRequestHandler = (
  overrides: Partial<A2ARequestHandler> = {},
): A2ARequestHandler => ({
  async getAgentCard() {
    return agentCard;
  },
  async getAuthenticatedExtendedAgentCard() {
    return agentCard;
  },
  async sendMessage() {
    throw A2AError.unsupportedOperation('not used');
  },
  async *sendMessageStream() {
    throw A2AError.unsupportedOperation('not used');
  },
  async getTask(params) {
    return canceledTask(params.id, 'context-1');
  },
  async cancelTask(params) {
    return canceledTask(params.id, 'context-1');
  },
  async setTaskPushNotificationConfig(params) {
    return params;
  },
  async getTaskPushNotificationConfig(params) {
    return { taskId: params.id, pushNotificationConfig: { url: 'https://example.invalid' } };
  },
  async listTaskPushNotificationConfigs() {
    return [];
  },
  async deleteTaskPushNotificationConfig() {},
  async *resubscribe() {},
  ...overrides,
});

const agentCard: AgentCard = {
  name: 'doric',
  description: 'Doric',
  protocolVersion: '0.3.0',
  url: 'http://localhost/rpc',
  preferredTransport: 'JSONRPC',
  version: '0.0.1',
  capabilities: { streaming: true, pushNotifications: true },
  defaultInputModes: ['text'],
  defaultOutputModes: ['text'],
  skills: [],
};

const unauthenticatedUser = {
  isAuthenticated: false,
  userName: 'anonymous',
};

const canceledTask = (id: string, contextId: string): Task => ({
  kind: 'task',
  id,
  contextId,
  status: { state: 'canceled' },
  history: [],
});

const createDisposableSandbox = () =>
  ({
    id: 'sandbox-1',
    root: '/workspace',
    disposed: false,
    async exec() {
      throw new Error('not used');
    },
    async cloneRepo() {
      throw new Error('not used');
    },
    async readFile() {
      throw new Error('not used');
    },
    async writeFile() {},
    async putFile() {},
    async getFile() {
      throw new Error('not used');
    },
    async diff() {
      throw new Error('not used');
    },
    async dispose() {
      this.disposed = true;
    },
  }) as DoricSessionContext['sandbox'] & { disposed: boolean };

const isTask = (value: unknown): value is Task =>
  typeof value === 'object' &&
  value !== null &&
  'kind' in value &&
  value.kind === 'task';
