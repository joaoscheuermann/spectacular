import assert from 'node:assert/strict';
import test from 'node:test';

import { createTool, createToolStorage } from 'tools';
import { z } from 'zod';

import {
  ProviderErrorObject,
  createOpenAiProvider,
  openAiBody,
  type ProviderStreamEvent,
} from '../src/index.js';
import { collect, fakeTransport, response } from './fakes.js';

test('maps OpenAI Responses DTO with instructions tools reasoning and fast service tier', () => {
  const body = openAiBody(
    {
      model: 'gpt-5-fast',
      messages: [
        { role: 'system', content: 'Follow policy.' },
        { role: 'user', content: 'Plan it.' },
        { role: 'tool', toolCallId: 'call_1', content: 'tool output' },
      ],
      tools: [
        {
          name: 'search',
          description: 'Search docs',
          inputSchema: { type: 'object', properties: {} },
          strict: true,
        },
      ],
      maxOutputTokens: 100,
      flags: { reasoning: { effort: 'low', summary: 'concise' } },
    },
    false,
  );

  assert.equal(body.model, 'gpt-5');
  assert.equal(body.instructions, 'Follow policy.');
  assert.equal(body.service_tier, 'priority');
  assert.deepEqual(body.reasoning, { effort: 'low', summary: 'concise' });
  assert.deepEqual(body.tools, [
    {
      type: 'function',
      name: 'search',
      description: 'Search docs',
      parameters: { type: 'object', properties: {} },
      strict: true,
    },
  ]);
  assert.deepEqual(body.input, [
    {
      role: 'user',
      content: [{ type: 'input_text', text: 'Plan it.' }],
    },
    {
      type: 'function_call_output',
      call_id: 'call_1',
      output: 'tool output',
    },
  ]);
  assert.equal('text' in body, false);
});

test('maps OpenAI structured output schemas to text format DTOs', () => {
  const body = openAiBody(
    {
      model: 'gpt-5',
      messages: [{ role: 'user', content: 'Return JSON.' }],
      schema: z.object({ answer: z.string() }),
    },
    false,
  );

  assert.deepEqual(body.text, {
    format: {
      type: 'json_schema',
      name: 'structured_output',
      strict: true,
      schema: {
        $schema: 'https://json-schema.org/draft/2020-12/schema',
        type: 'object',
        properties: { answer: { type: 'string' } },
        required: ['answer'],
        additionalProperties: false,
      },
    },
  });
});

test('accepts tool definitions from shared tool storage', () => {
  const tools = createToolStorage([
    createTool({
      name: 'lookup',
      description: 'Lookup context',
      schema: z.object({ query: z.string() }),
      execute: ({ query }) => query,
    }),
  ]);

  const body = openAiBody(
    {
      model: 'gpt-5',
      messages: [{ role: 'user', content: 'Use the tool.' }],
      tools: tools.definitions(),
    },
    false,
  );

  assert.deepEqual(body.tools, [
    {
      type: 'function',
      name: 'lookup',
      description: 'Lookup context',
      parameters: tools.definitions()[0]?.inputSchema,
      strict: true,
    },
  ]);
});

test('rejects OpenAI requests that are missing model or input', async () => {
  const provider = createOpenAiProvider({
    transport: fakeTransport({}),
    apiKey: 'sk-testSecret123',
  });

  await assert.rejects(
    provider.complete({
      model: '',
      messages: [{ role: 'user', content: 'hi' }],
    }),
    (error: unknown) =>
      error instanceof ProviderErrorObject &&
      error.data.code === 'missing_model',
  );

  await assert.rejects(
    provider.complete({ model: 'gpt-5', messages: [] }),
    (error: unknown) =>
      error instanceof ProviderErrorObject &&
      error.data.code === 'missing_input',
  );
});

test('parses OpenAI completion output usage reasoning and tool calls', async () => {
  const transport = fakeTransport({
    responses: [
      response({
        status: 'completed',
        output_text: 'Done',
        output: [
          { type: 'reasoning', summary: [{ text: 'Thought' }] },
          {
            type: 'function_call',
            call_id: 'call_1',
            name: 'lookup',
            arguments: '{"q":"x"}',
          },
        ],
        usage: {
          input_tokens: 3,
          output_tokens: 4,
          total_tokens: 7,
          output_tokens_details: { reasoning_tokens: 2 },
        },
      }),
    ],
  });
  const provider = createOpenAiProvider({
    transport,
    apiKey: 'sk-testSecret123',
  });

  const result = await provider.complete({
    model: 'gpt-5',
    messages: [{ role: 'user', content: 'Hi' }],
  });

  assert.equal(result.text, 'Done');
  assert.equal(result.finishReason, 'stop');
  assert.deepEqual(result.reasoning, { text: 'Thought' });
  assert.deepEqual(result.usage, {
    inputTokens: 3,
    outputTokens: 4,
    totalTokens: 7,
    reasoningTokens: 2,
    cachedInputTokens: undefined,
  });
  assert.deepEqual(result.toolCalls, [
    { id: 'call_1', name: 'lookup', arguments: '{"q":"x"}', index: 0 },
  ]);
});

test('returns parsed OpenAI structured output from completions', async () => {
  const transport = fakeTransport({
    responses: [
      response({
        status: 'completed',
        output_text: '{"answer":"Done"}',
        output: [],
      }),
    ],
  });
  const provider = createOpenAiProvider({
    transport,
    apiKey: 'sk-testSecret123',
  });

  const result = await provider.complete({
    model: 'gpt-5',
    messages: [{ role: 'user', content: 'Hi' }],
    schema: z.object({ answer: z.string() }),
  });

  assert.deepEqual(result.structured, { answer: 'Done' });
});

test('returns OpenAI refusals without structured parsing', async () => {
  const transport = fakeTransport({
    responses: [
      response({
        status: 'completed',
        output: [
          {
            type: 'message',
            content: [{ type: 'refusal', refusal: 'No.' }],
          },
        ],
      }),
    ],
  });
  const provider = createOpenAiProvider({
    transport,
    apiKey: 'sk-testSecret123',
  });

  const result = await provider.complete({
    model: 'gpt-5',
    messages: [{ role: 'user', content: 'Hi' }],
    schema: z.object({ answer: z.string() }),
  });

  assert.equal(result.refusal, 'No.');
  assert.equal(result.structured, undefined);
});

test('sends OpenAI API key auth as bearer token', async () => {
  const transport = fakeTransport({
    responses: [
      response({ status: 'completed', output_text: 'ok', output: [] }),
    ],
  });
  const provider = createOpenAiProvider({
    transport,
    apiKey: 'sk-testSecret123',
  });

  await provider.complete({
    model: 'gpt-5',
    messages: [{ role: 'user', content: 'Hi' }],
  });

  assert.equal(
    transport.requests[0]?.headers?.authorization,
    'Bearer sk-testSecret123',
  );
});

test('sends exact OpenAI authorization header when supplied', async () => {
  const transport = fakeTransport({
    responses: [
      response({ status: 'completed', output_text: 'ok', output: [] }),
    ],
  });
  const provider = createOpenAiProvider({
    transport,
    authorization: 'Custom credential-value',
  });

  await provider.complete({
    model: 'gpt-5',
    messages: [{ role: 'user', content: 'Hi' }],
  });

  assert.equal(
    transport.requests[0]?.headers?.authorization,
    'Custom credential-value',
  );
});

test('rejects ambiguous OpenAI auth configuration', async () => {
  const provider = createOpenAiProvider({
    transport: fakeTransport({}),
    apiKey: 'sk-testSecret123',
    authorization: 'Bearer token',
  });

  await assert.rejects(
    provider.complete({
      model: 'gpt-5',
      messages: [{ role: 'user', content: 'Hi' }],
    }),
    (error: unknown) =>
      error instanceof ProviderErrorObject &&
      error.data.code === 'auth_ambiguous',
  );
});

test('streams OpenAI text reasoning usage finish and tool calls', async () => {
  const stream = [
    sse({ type: 'response.output_text.delta', delta: 'Hel' }),
    sse({ type: 'response.output_text.delta', delta: 'lo' }),
    sse({ type: 'response.reasoning_summary_text.delta', delta: 'why' }),
    sse({
      type: 'response.function_call_arguments.delta',
      output_index: 0,
      item_id: 'call_1',
      name: 'lookup',
      delta: '{"q"',
    }),
    sse({
      type: 'response.function_call_arguments.delta',
      output_index: 0,
      delta: ':"x"}',
    }),
    sse({
      type: 'response.output_item.done',
      output_index: 0,
      item: {
        type: 'function_call',
        call_id: 'call_1',
        name: 'lookup',
        arguments: '{"q":"x"}',
      },
    }),
    sse({
      type: 'response.completed',
      response: {
        status: 'completed',
        output_text: 'Hello',
        usage: { input_tokens: 1, output_tokens: 2, total_tokens: 3 },
      },
    }),
  ];
  const provider = createOpenAiProvider({
    transport: fakeTransport({ streams: [stream] }),
    apiKey: 'sk-testSecret123',
  });

  const events = await collect(
    provider.stream({
      model: 'gpt-5',
      messages: [{ role: 'user', content: 'Hi' }],
    }),
  );

  assert.equal(events[0]?.type, 'response.started');
  assert.deepEqual(
    events
      .filter(
        (event): event is ProviderStreamEvent & { type: 'text.delta' } =>
          event.type === 'text.delta',
      )
      .map((event) => event.delta),
    ['Hel', 'lo'],
  );
  assert.ok(events.some((event) => event.type === 'reasoning.delta'));
  assert.ok(events.some((event) => event.type === 'tool_call.done'));
  assert.ok(events.some((event) => event.type === 'usage'));
  assert.equal(events.at(-1)?.type, 'response.finished');
});

test('returns parsed OpenAI structured output from stream finishes', async () => {
  const provider = createOpenAiProvider({
    transport: fakeTransport({
      streams: [
        [
          sse({ type: 'response.output_text.delta', delta: '{"answer"' }),
          sse({ type: 'response.output_text.delta', delta: ':"Done"}' }),
          sse({
            type: 'response.completed',
            response: {
              status: 'completed',
              output_text: '{"answer":"Done"}',
              usage: { input_tokens: 1, output_tokens: 2, total_tokens: 3 },
            },
          }),
        ],
      ],
    }),
    apiKey: 'sk-testSecret123',
  });

  const events = await collect(
    provider.stream({
      model: 'gpt-5',
      messages: [{ role: 'user', content: 'Hi' }],
      schema: z.object({ answer: z.string() }),
    }),
  );
  const finished = events.at(-1);

  assert.equal(finished?.type, 'response.finished');

  if (finished?.type !== 'response.finished') {
    assert.fail('Expected final response.finished event.');
  }

  assert.deepEqual(finished.finish.structured, { answer: 'Done' });
});

test('returns OpenAI stream refusals without structured parsing', async () => {
  const provider = createOpenAiProvider({
    transport: fakeTransport({
      streams: [
        [
          sse({ type: 'response.refusal.delta', delta: 'No.' }),
          sse({
            type: 'response.completed',
            response: {
              status: 'completed',
            },
          }),
        ],
      ],
    }),
    apiKey: 'sk-testSecret123',
  });

  const events = await collect(
    provider.stream({
      model: 'gpt-5',
      messages: [{ role: 'user', content: 'Hi' }],
      schema: z.object({ answer: z.string() }),
    }),
  );
  const finished = events.at(-1);

  assert.equal(finished?.type, 'response.finished');

  if (finished?.type !== 'response.finished') {
    assert.fail('Expected final response.finished event.');
  }

  assert.equal(finished.finish.refusal, 'No.');
  assert.equal(finished.finish.structured, undefined);
});

test('rejects invalid OpenAI structured JSON', async () => {
  const completeProvider = createOpenAiProvider({
    transport: fakeTransport({
      responses: [
        response({
          status: 'completed',
          output_text: 'not-json',
          output: [],
        }),
      ],
    }),
    apiKey: 'sk-testSecret123',
  });
  const streamProvider = createOpenAiProvider({
    transport: fakeTransport({
      streams: [
        [
          sse({
            type: 'response.completed',
            response: {
              status: 'completed',
              output_text: 'not-json',
            },
          }),
        ],
      ],
    }),
    apiKey: 'sk-testSecret123',
  });
  const request = {
    model: 'gpt-5',
    messages: [{ role: 'user', content: 'Hi' }],
    schema: z.object({ answer: z.string() }),
  } as const;

  await assert.rejects(
    completeProvider.complete(request),
    (error: unknown) =>
      error instanceof ProviderErrorObject &&
      error.data.code === 'invalid_structured_output',
  );
  await assert.rejects(
    collect(streamProvider.stream(request)),
    (error: unknown) =>
      error instanceof ProviderErrorObject &&
      error.data.code === 'invalid_structured_output',
  );
});

test('rejects schema-invalid OpenAI structured JSON', async () => {
  const provider = createOpenAiProvider({
    transport: fakeTransport({
      responses: [
        response({
          status: 'completed',
          output_text: '{"answer":123}',
          output: [],
        }),
      ],
    }),
    apiKey: 'sk-testSecret123',
  });

  await assert.rejects(
    provider.complete({
      model: 'gpt-5',
      messages: [{ role: 'user', content: 'Hi' }],
      schema: z.object({ answer: z.string() }),
    }),
    (error: unknown) =>
      error instanceof ProviderErrorObject &&
      error.data.code === 'invalid_structured_output' &&
      error.data.diagnostic?.includes('answer') === true,
  );
});

test('does not refresh OpenAI auth after 401 responses', async () => {
  const transport = fakeTransport({
    responses: [response({ error: 'expired' }, 401)],
  });
  const provider = createOpenAiProvider({
    transport,
    authorization: 'Bearer expired-token',
  });

  await assert.rejects(
    provider.complete({
      model: 'gpt-5',
      messages: [{ role: 'user', content: 'Hi' }],
    }),
    (error: unknown) =>
      error instanceof ProviderErrorObject && error.data.code === 'auth_failed',
  );
  assert.equal(transport.requests.length, 1);
});

test('returns stream error event for malformed OpenAI stream payloads', async () => {
  const provider = createOpenAiProvider({
    transport: fakeTransport({ streams: [['data: not-json\n\n']] }),
    apiKey: 'sk-testSecret123',
  });

  const events = await collect(
    provider.stream({
      model: 'gpt-5',
      messages: [{ role: 'user', content: 'Hi' }],
    }),
  );

  assert.equal(events.at(-1)?.type, 'error');
});

const sse = (value: unknown): string => `data: ${JSON.stringify(value)}\n\n`;
