import assert from 'node:assert/strict';
import test from 'node:test';

import { z } from 'zod';
import type { Sandbox } from 'sandbox';

import {
  ToolErrorObject,
  createToolStorage,
  defineTool,
  type ToolCall,
} from '../src/index.js';

const sandbox = { id: 'sandbox', root: '/workspace' } as Sandbox;

test('infers typed payloads from Zod schemas at compile time', async () => {
  const expectString = (value: string): string => value;
  const expectNumber = (value: number): number => value;
  const factory = defineTool({
    name: 'lookup',
    schema: z.object({
      query: z.string(),
      limit: z.number().optional(),
    }),
    execute(received, payload) {
      assert.equal(received, sandbox);
      expectString(payload.query);

      if (payload.limit !== undefined) {
        expectNumber(payload.limit);
      }

      // @ts-expect-error query is inferred as string, not number.
      expectNumber(payload.query);

      return { query: payload.query, limit: payload.limit };
    },
  });
  const tool = factory(sandbox);

  assert.deepEqual(await tool.execute({ query: 'doric' }), {
    query: 'doric',
    limit: undefined,
  });
});

test('exposes metadata before binding and binds the supplied sandbox', async () => {
  const factory = defineTool({
    name: 'search',
    description: 'Search indexed context.',
    schema: z.object({ query: z.string(), limit: z.number().int().min(1) }),
    execute: (received, { query, limit }) => {
      assert.equal(received, sandbox);
      return `${query}:${limit}`;
    },
  });

  assert.equal(factory.name, 'search');
  assert.equal(factory.description, 'Search indexed context.');
  assert.equal(factory.schema instanceof z.ZodObject, true);
  assert.equal(factory.definition.name, 'search');
  assert.equal(factory.definition.description, 'Search indexed context.');
  assert.equal(factory.definition.strict, true);
  assert.equal(factory.definition.inputSchema.type, 'object');
  assert.deepEqual(factory.definition.inputSchema.required, ['query', 'limit']);
  assert.deepEqual(factory.definition.inputSchema.properties, {
    query: { type: 'string' },
    limit: { type: 'integer', minimum: 1, maximum: Number.MAX_SAFE_INTEGER },
  });
  assert.equal(await factory(sandbox).execute({ query: 'x', limit: 2 }), 'x:2');
});

test('allows non-strict definitions when requested', () => {
  const tool = defineTool({
    name: 'draft',
    schema: z.object({ value: z.string() }),
    strict: false,
    execute: (_sandbox, { value }) => value,
  });

  assert.equal(tool.definition.strict, false);
});

test('rejects non-object and unrepresentable schemas', () => {
  assert.throws(
    () =>
      defineTool({
        name: 'bad',
        schema: z.string() as unknown as z.ZodObject,
        execute: () => undefined,
      }),
    (error: unknown) =>
      error instanceof ToolErrorObject && error.data.code === 'invalid_schema',
  );

  assert.throws(
    () =>
      defineTool({
        name: 'bad-date',
        schema: z.object({ at: z.date() }),
        execute: () => undefined,
      }),
    (error: unknown) =>
      error instanceof ToolErrorObject && error.data.code === 'invalid_schema',
  );
});

test('preserves definition order and rejects duplicate names', () => {
  const first = defineTool({
    name: 'first',
    schema: z.object({}),
    execute: () => 'first',
  });
  const second = defineTool({
    name: 'second',
    schema: z.object({}),
    execute: () => 'second',
  });

  assert.deepEqual(
    createToolStorage([first(sandbox), second(sandbox)])
      .definitions()
      .map((definition) => definition.name),
    ['first', 'second'],
  );

  assert.throws(
    () => createToolStorage([first(sandbox), first(sandbox)]),
    (error: unknown) =>
      error instanceof ToolErrorObject && error.data.code === 'duplicate_tool',
  );
});

test('parses provider tool-call arguments into payloads', () => {
  const storage = createToolStorage([]);

  assert.deepEqual(
    storage.calls({
      toolCalls: [
        {
          id: 'call_1',
          name: 'lookup',
          arguments: '{"query":"doric"}',
          index: 0,
        },
      ],
    }),
    [{ id: 'call_1', name: 'lookup', payload: { query: 'doric' }, index: 0 }],
  );
});

test('validates payloads before execution and supports async handlers', async () => {
  const storage = createToolStorage([
    defineTool({
      name: 'add',
      schema: z.object({ left: z.number(), right: z.number() }),
      async execute(_sandbox, { left, right }) {
        return left + right;
      },
    })(sandbox),
  ]);

  assert.equal(
    await storage.execute({
      id: 'call_1',
      name: 'add',
      payload: { left: 2, right: 3 },
    }),
    5,
  );

  await assert.rejects(
    storage.execute({
      id: 'call_2',
      name: 'add',
      payload: { left: '2', right: 3 },
    } satisfies ToolCall),
    (error: unknown) =>
      error instanceof ToolErrorObject &&
      error.data.code === 'invalid_payload' &&
      error.data.issues?.[0]?.path === 'left',
  );
});

test('throws typed errors for unknown tools invalid JSON and handler failures', async () => {
  const storage = createToolStorage([
    defineTool({
      name: 'explode',
      schema: z.object({ value: z.string() }),
      execute() {
        throw new Error('boom');
      },
    })(sandbox),
  ]);

  await assert.rejects(
    storage.execute({
      id: 'call_1',
      name: 'missing',
      arguments: '{}',
    }),
    (error: unknown) =>
      error instanceof ToolErrorObject && error.data.code === 'unknown_tool',
  );

  await assert.rejects(
    storage.execute({
      id: 'call_2',
      name: 'explode',
      arguments: '{',
    }),
    (error: unknown) =>
      error instanceof ToolErrorObject && error.data.code === 'invalid_json',
  );

  await assert.rejects(
    storage.execute({
      id: 'call_3',
      name: 'explode',
      arguments: '{"value":"x"}',
    }),
    (error: unknown) =>
      error instanceof ToolErrorObject && error.data.code === 'handler_failed',
  );
});
