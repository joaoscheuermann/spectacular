import assert from 'node:assert/strict';
import test from 'node:test';

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
});

test('rejects OpenAI requests that are missing model or input', async () => {
  const provider = createOpenAiProvider({
    transport: fakeTransport({}),
    apiKey: 'sk-testSecret123',
  });

  await assert.rejects(
    provider.complete({ model: '', messages: [{ role: 'user', content: 'hi' }] }),
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
  const provider = createOpenAiProvider({ transport, apiKey: 'sk-testSecret123' });

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
    events.filter((event): event is ProviderStreamEvent & { type: 'text.delta' } => event.type === 'text.delta')
      .map((event) => event.delta),
    ['Hel', 'lo'],
  );
  assert.ok(events.some((event) => event.type === 'reasoning.delta'));
  assert.ok(events.some((event) => event.type === 'tool_call.done'));
  assert.ok(events.some((event) => event.type === 'usage'));
  assert.equal(events.at(-1)?.type, 'response.finished');
});

test('retries OpenAI OAuth request after 401 by refreshing before the second call', async () => {
  const saved: unknown[] = [];
  const transport = fakeTransport({
    responses: [
      response({ error: 'expired' }, 401),
      response({ access_token: 'next.token.value', expires_in: 3600 }),
      response({ status: 'completed', output_text: 'ok', output: [] }),
    ],
  });
  const provider = createOpenAiProvider({
    transport,
    authStore: {
      async load() {
        return {
          accessToken: 'old.token.value',
          refreshToken: 'refresh-token',
          expiresAt: Date.now() + 120_000,
        };
      },
      async save(record) {
        saved.push(record);
      },
    },
    clientId: 'client',
  });

  const result = await provider.complete({
    model: 'gpt-5',
    messages: [{ role: 'user', content: 'Hi' }],
  });

  assert.equal(result.text, 'ok');
  assert.equal(transport.requests.length, 3);
  assert.equal(transport.requests[1]?.url, 'https://auth.openai.com/oauth/token');
  assert.equal(saved.length, 1);
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
