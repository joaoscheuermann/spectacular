import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import type { Server } from 'node:http';
import test from 'node:test';

import type { Message, Task } from '@a2a-js/sdk';
import { ClientFactory } from '@a2a-js/sdk/client';

import { createServer, HELLO_WORLD_TEXT } from '../src/index.js';
import { createConfigPart, createDoricTestHarness } from './fakes.js';

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u;

test('serves the A2A Agent Card at the well-known path', async () => {
  await withServer(async (origin) => {
    const response = await fetch(`${origin}/.well-known/agent-card.json`);

    assert.equal(response.status, 200);
    assert.equal(
      response.headers.get('content-type'),
      'application/json; charset=utf-8',
    );
    assert.deepEqual(await response.json(), {
      name: 'doric',
      description: 'Doric A2A Coding Agent.',
      protocolVersion: '0.3.0',
      url: `${origin}/rpc`,
      preferredTransport: 'JSONRPC',
      additionalInterfaces: [{ url: `${origin}/rpc`, transport: 'JSONRPC' }],
      provider: {
        organization: 'Doric',
        url: 'https://github.com/joaoscheuermann/spectacular',
      },
      version: '0.0.1',
      capabilities: {
        pushNotifications: false,
        streaming: false,
      },
      defaultInputModes: ['text'],
      defaultOutputModes: ['text'],
      skills: [
        {
          id: 'hello-world',
          name: 'Hello world',
          description: 'Returns a hello world message.',
          tags: ['hello-world', 'text'],
          inputModes: ['text'],
          outputModes: ['text'],
        },
      ],
      supportsAuthenticatedExtendedCard: false,
    });
  });
});

test('returns a hello world message through the SDK client', async () => {
  await withServer(async (origin) => {
    const client = await new ClientFactory().createFromUrl(origin);
    const result = await client.sendMessage({
      message: {
        kind: 'message',
        messageId: randomUUID(),
        role: 'user',
        contextId: 'context-123',
        parts: [createConfigPart(), { kind: 'text', text: 'Hello?' }],
      },
    });

    const message = assertMessage(result);

    assert.match(message.messageId, UUID_PATTERN);
    assert.equal(message.contextId, 'context-123');
    assert.equal(message.role, 'agent');
    assert.deepEqual(message.parts, [
      {
        kind: 'text',
        text: HELLO_WORLD_TEXT,
      },
    ]);
  });
});

test('returns a hello world message when JSON-RPC message send is posted', async () => {
  await withServer(async (origin) => {
    const response = await postJson(origin, {
      jsonrpc: '2.0',
      id: 'send-message',
      method: 'message/send',
      params: {
        message: {
          kind: 'message',
          messageId: randomUUID(),
          role: 'user',
          contextId: 'context-123',
          parts: [createConfigPart(), { kind: 'text', text: 'Hello?' }],
        },
      },
    });

    assert.equal(response.status, 200);
    const body = (await response.json()) as JsonRpcResponse;

    assert.equal(body.jsonrpc, '2.0');
    assert.equal(body.id, 'send-message');
    const message = assertJsonRpcMessage(body);

    assert.equal(message.contextId, 'context-123');
    assert.deepEqual(message.parts, [
      {
        kind: 'text',
        text: HELLO_WORLD_TEXT,
      },
    ]);
  });
});

test('returns a hello world message when the SDK generates a context ID', async () => {
  await withServer(async (origin) => {
    const response = await postJson(origin, {
      jsonrpc: '2.0',
      id: 'send-message',
      method: 'message/send',
      params: {
        message: {
          kind: 'message',
          messageId: randomUUID(),
          role: 'user',
          parts: [createConfigPart(), { kind: 'text', text: 'Hello?' }],
        },
      },
    });

    assert.equal(response.status, 200);

    const body = (await response.json()) as JsonRpcResponse;
    const message = assertJsonRpcMessage(body);

    assert.match(message.contextId ?? '', UUID_PATTERN);
    assert.deepEqual(message.parts, [
      {
        kind: 'text',
        text: HELLO_WORLD_TEXT,
      },
    ]);
  });
});

test('returns a failed task when the first message part is text', async () => {
  await withServer(async (origin) => {
    const response = await postJson(origin, {
      jsonrpc: '2.0',
      id: 'send-message',
      method: 'message/send',
      params: {
        message: {
          kind: 'message',
          messageId: randomUUID(),
          role: 'user',
          contextId: 'context-123',
          parts: [{ kind: 'text', text: 'Hello?' }],
        },
      },
    });

    assert.equal(response.status, 200);
    const body = (await response.json()) as JsonRpcResponse;
    const task = assertJsonRpcTask(body);

    assert.equal(body.jsonrpc, '2.0');
    assert.equal(body.id, 'send-message');
    assert.equal(task.status.state, 'failed');
    assert.equal(task.status.message?.parts[0]?.kind, 'text');
    assert.match(
      task.status.message?.parts[0]?.kind === 'text'
        ? task.status.message.parts[0].text
        : '',
      /First message part must be a data part/u,
    );
  });
});

test('returns method not found when JSON-RPC method is unsupported', async () => {
  await withServer(async (origin) => {
    const response = await postJson(origin, {
      jsonrpc: '2.0',
      id: 2,
      method: 'tasks/unknown',
      params: {},
    });

    assert.equal(response.status, 200);
    const body = (await response.json()) as JsonRpcResponse;

    assert.equal(body.jsonrpc, '2.0');
    assert.equal(body.id, 2);
    assert.equal(assertJsonRpcError(body).code, -32601);
  });
});

const postJson = async (origin: string, body: unknown): Promise<Response> =>
  fetch(`${origin}/rpc`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
    },
    body: JSON.stringify(body),
  });

type JsonRpcResponse = {
  readonly jsonrpc: '2.0';
  readonly id: unknown;
  readonly result?: Message | Task;
  readonly error?: {
    readonly code: number;
    readonly data?: Record<string, unknown>;
  };
};

const assertMessage = (result: Message | Task): Message => {
  if (result.kind !== 'message') {
    assert.fail('expected direct message response');
  }

  return result;
};

const assertJsonRpcMessage = (body: JsonRpcResponse): Message => {
  if (body.result === undefined) {
    assert.fail('expected JSON-RPC result response');
  }

  return assertMessage(body.result);
};

const assertJsonRpcTask = (body: JsonRpcResponse): Task => {
  if (body.result === undefined) {
    assert.fail('expected JSON-RPC result response');
  }

  if (body.result.kind !== 'task') {
    assert.fail('expected JSON-RPC task response');
  }

  return body.result;
};

const assertJsonRpcError = (
  body: JsonRpcResponse,
): NonNullable<JsonRpcResponse['error']> => {
  if (body.error === undefined) {
    assert.fail('expected JSON-RPC error response');
  }

  return body.error;
};

const withServer = async (
  run: (origin: string) => Promise<void>,
): Promise<void> => {
  const harness = createDoricTestHarness();
  const server = createServer({ executor: harness.executor });
  await listen(server);

  const address = server.address();
  assert.ok(address !== null && typeof address === 'object');

  try {
    await run(`http://127.0.0.1:${address.port}`);
  } finally {
    await close(server);
  }
};

const listen = async (server: Server): Promise<void> =>
  new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });

const close = async (server: Server): Promise<void> =>
  new Promise((resolve, reject) => {
    server.close((error) => {
      if (error) {
        reject(error);
        return;
      }

      resolve();
    });
  });
