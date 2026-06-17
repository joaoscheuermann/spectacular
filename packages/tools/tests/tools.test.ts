import assert from 'node:assert/strict';
import test from 'node:test';

import { z } from 'zod';

import {
  ToolErrorObject,
  createTool,
  createToolStorage,
  type ToolCall,
} from '../src/index.js';

test('infers typed payloads from Zod schemas at compile time', async () => {
  const expectString = (value: string): string => value;
  const expectNumber = (value: number): number => value;
  const tool = createTool({
    name: 'lookup',
    schema: z.object({
      query: z.string(),
      limit: z.number().optional(),
    }),
    execute(payload) {
      expectString(payload.query);

      if (payload.limit !== undefined) {
        expectNumber(payload.limit);
      }

      // @ts-expect-error query is inferred as string, not number.
      expectNumber(payload.query);

      return { query: payload.query, limit: payload.limit };
    },
  });

  assert.deepEqual(await tool.execute({ query: 'doric' }), {
    query: 'doric',
    limit: undefined,
  });
});

test('emits JSON Schema definitions from Zod object schemas', () => {
  const tool = createTool({
    name: 'search',
    description: 'Search indexed context.',
    schema: z.object({ query: z.string(), limit: z.number().int().min(1) }),
    execute: ({ query, limit }) => `${query}:${limit}`,
  });

  assert.equal(tool.definition.name, 'search');
  assert.equal(tool.definition.description, 'Search indexed context.');
  assert.equal(tool.definition.strict, true);
  assert.equal(tool.definition.inputSchema.type, 'object');
  assert.deepEqual(tool.definition.inputSchema.required, ['query', 'limit']);
  assert.deepEqual(tool.definition.inputSchema.properties, {
    query: { type: 'string' },
    limit: { type: 'integer', minimum: 1, maximum: Number.MAX_SAFE_INTEGER },
  });
});

test('allows non-strict definitions when requested', () => {
  const tool = createTool({
    name: 'draft',
    schema: z.object({ value: z.string() }),
    strict: false,
    execute: ({ value }) => value,
  });

  assert.equal(tool.definition.strict, false);
});

test('rejects non-object and unrepresentable schemas', () => {
  assert.throws(
    () =>
      createTool({
        name: 'bad',
        schema: z.string() as unknown as z.ZodObject,
        execute: () => undefined,
      }),
    (error: unknown) =>
      error instanceof ToolErrorObject && error.data.code === 'invalid_schema',
  );

  assert.throws(
    () =>
      createTool({
        name: 'bad-date',
        schema: z.object({ at: z.date() }),
        execute: () => undefined,
      }),
    (error: unknown) =>
      error instanceof ToolErrorObject && error.data.code === 'invalid_schema',
  );
});

test('preserves definition order and rejects duplicate names', () => {
  const first = createTool({
    name: 'first',
    schema: z.object({}),
    execute: () => 'first',
  });
  const second = createTool({
    name: 'second',
    schema: z.object({}),
    execute: () => 'second',
  });

  assert.deepEqual(
    createToolStorage([first, second])
      .definitions()
      .map((definition) => definition.name),
    ['first', 'second'],
  );

  assert.throws(
    () => createToolStorage([first, first]),
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
    createTool({
      name: 'add',
      schema: z.object({ left: z.number(), right: z.number() }),
      async execute({ left, right }) {
        return left + right;
      },
    }),
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
    createTool({
      name: 'explode',
      schema: z.object({ value: z.string() }),
      execute() {
        throw new Error('boom');
      },
    }),
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
