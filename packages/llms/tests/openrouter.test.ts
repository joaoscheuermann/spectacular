import assert from 'node:assert/strict';
import test from 'node:test';

import {
  ProviderErrorObject,
  createOpenRouterProvider,
  openRouterBody,
  type ProviderStreamEvent,
} from '../src/index.js';
import { collect, fakeTransport, response } from './fakes.js';

test('maps OpenRouter chat completions DTO with messages tools and reasoning', () => {
  const body = openRouterBody(
    {
      model: 'openai/gpt-5',
      messages: [
        { role: 'system', content: 'System' },
        { role: 'assistant', content: 'Use tool', toolCalls: [{ id: 'call_1', name: 'lookup', arguments: '{}', index: 0 }] },
        { role: 'tool', toolCallId: 'call_1', content: 'Result' },
      ],
      tools: [{ name: 'lookup', inputSchema: { type: 'object' } }],
      temperature: 0.1,
      maxOutputTokens: 32,
      flags: { reasoning: { effort: 'medium' } },
    },
    false,
  );

  assert.equal(body.model, 'openai/gpt-5');
  assert.equal(body.stream, false);
  assert.equal(body.max_tokens, 32);
  assert.deepEqual(body.reasoning, { effort: 'medium' });
  assert.deepEqual(body.tools, [
    {
      type: 'function',
      function: {
        name: 'lookup',
        parameters: { type: 'object' },
      },
    },
  ]);
  assert.deepEqual(body.messages, [
    { role: 'system', content: 'System' },
    {
      role: 'assistant',
      content: 'Use tool',
      tool_calls: [
        {
          id: 'call_1',
          type: 'function',
          function: { name: 'lookup', arguments: '{}' },
        },
      ],
    },
    { role: 'tool', content: 'Result', tool_call_id: 'call_1' },
  ]);
});

test('parses OpenRouter completion and redacts auth failures', async () => {
  const transport = fakeTransport({
    responses: [
      response({
        choices: [
          {
            finish_reason: 'tool_calls',
            message: {
              content: 'Hello',
              reasoning: 'Because',
              refusal: 'No',
              tool_calls: [
                {
                  id: 'call_1',
                  function: { name: 'lookup', arguments: '{"q":"x"}' },
                },
              ],
            },
          },
        ],
        usage: {
          prompt_tokens: 2,
          completion_tokens: 3,
          total_tokens: 5,
          completion_tokens_details: { reasoning_tokens: 1 },
          prompt_tokens_details: { cached_tokens: 1 },
        },
      }),
      response({ error: { message: 'bad sk-testSecret123' } }, 401),
      response({ error: { message: 'forbidden sk-testSecret456' } }, 403),
    ],
  });
  const provider = createOpenRouterProvider({ transport, apiKey: 'sk-testSecret123' });

  const result = await provider.complete({
    model: 'openai/gpt-5',
    messages: [{ role: 'user', content: 'Hi' }],
  });

  assert.equal(result.text, 'Hello');
  assert.equal(result.finishReason, 'tool_calls');
  assert.deepEqual(result.reasoning, { text: 'Because' });
  assert.equal(result.refusal, 'No');
  assert.deepEqual(result.toolCalls, [
    { id: 'call_1', name: 'lookup', arguments: '{"q":"x"}', index: 0 },
  ]);
  assert.deepEqual(result.usage, {
    inputTokens: 2,
    outputTokens: 3,
    totalTokens: 5,
    reasoningTokens: 1,
    cachedInputTokens: 1,
  });

  await assert.rejects(
    provider.complete({
      model: 'openai/gpt-5',
      messages: [{ role: 'user', content: 'Hi' }],
    }),
    (error: unknown) =>
      error instanceof ProviderErrorObject &&
      error.data.code === 'auth_failed' &&
      error.data.diagnostic?.includes('sk-[redacted]') === true,
  );

  await assert.rejects(
    provider.complete({
      model: 'openai/gpt-5',
      messages: [{ role: 'user', content: 'Hi' }],
    }),
    (error: unknown) =>
      error instanceof ProviderErrorObject &&
      error.data.code === 'auth_failed' &&
      error.data.status === 403 &&
      error.data.diagnostic?.includes('sk-[redacted]') === true,
  );
});

test('rejects OpenRouter requests that are missing model or input', async () => {
  const provider = createOpenRouterProvider({
    transport: fakeTransport({}),
    apiKey: 'key',
  });

  await assert.rejects(
    provider.complete({ model: '', messages: [{ role: 'user', content: 'Hi' }] }),
    (error: unknown) =>
      error instanceof ProviderErrorObject &&
      error.data.code === 'missing_model',
  );

  await assert.rejects(
    provider.complete({ model: 'openai/gpt-5', messages: [] }),
    (error: unknown) =>
      error instanceof ProviderErrorObject &&
      error.data.code === 'missing_input',
  );
});

test('fetches and validates OpenRouter models with context fallback', async () => {
  const provider = createOpenRouterProvider({
    transport: fakeTransport({
      responses: [
        response({
          data: [
            { id: 'a', name: 'A', context_length: 8192 },
            { id: 'b', top_provider: { context_length: 4096 } },
            { id: 'c' },
          ],
        }),
        response({ data: [{ id: 'b', top_provider: { context_length: 4096 } }] }),
      ],
    }),
    apiKey: 'key',
  });

  const models = await provider.models();
  assert.deepEqual(
    models.map((model) => [model.id, model.contextWindow]),
    [
      ['a', 8192],
      ['b', 4096],
      ['c', 4096],
    ],
  );
  assert.equal((await provider.validateModel('b')).contextWindow, 4096);
});

test('streams OpenRouter deltas usage finish and accumulated tool calls', async () => {
  const provider = createOpenRouterProvider({
    transport: fakeTransport({
      streams: [
        [
          sse({ choices: [{ delta: { content: 'Hi' } }] }),
          sse({ choices: [{ delta: { reasoning: 'why' } }] }),
          sse({ choices: [{ delta: { refusal: 'no' } }] }),
          sse({
            choices: [
              {
                delta: {
                  tool_calls: [
                    {
                      index: 0,
                      id: 'call_1',
                      function: { name: 'lookup', arguments: '{"q"' },
                    },
                  ],
                },
              },
            ],
          }),
          sse({
            choices: [
              {
                delta: {
                  tool_calls: [
                    {
                      index: 0,
                      function: { arguments: ':"x"}' },
                    },
                  ],
                },
                finish_reason: 'tool_calls',
              },
            ],
            usage: { prompt_tokens: 1, completion_tokens: 2, total_tokens: 3 },
          }),
          'data: [DONE]\n\n',
        ],
      ],
    }),
    apiKey: 'key',
  });

  const events = await collect(
    provider.stream({
      model: 'openai/gpt-5',
      messages: [{ role: 'user', content: 'Hi' }],
    }),
  );
  const finished = events.at(-1);
  const expectedToolCalls = [
    { id: 'call_1', name: 'lookup', arguments: '{"q":"x"}', index: 0 },
  ];

  assert.equal(events[0]?.type, 'response.started');
  assert.deepEqual(
    events
      .filter((event): event is Extract<ProviderStreamEvent, { type: 'text.delta' }> => event.type === 'text.delta')
      .map((event) => event.delta),
    ['Hi'],
  );
  assert.deepEqual(
    events
      .filter((event): event is Extract<ProviderStreamEvent, { type: 'reasoning.delta' }> => event.type === 'reasoning.delta')
      .map((event) => event.delta),
    ['why'],
  );
  assert.deepEqual(
    events
      .filter((event): event is Extract<ProviderStreamEvent, { type: 'refusal.delta' }> => event.type === 'refusal.delta')
      .map((event) => event.delta),
    ['no'],
  );
  assert.ok(events.some((event) => event.type === 'usage'));
  assert.deepEqual(
    events.filter((event): event is ProviderStreamEvent & { type: 'tool_call.done' } => event.type === 'tool_call.done')
      .map((event) => event.call),
    expectedToolCalls,
  );
  assert.equal(finished?.type, 'response.finished');

  if (finished?.type !== 'response.finished') {
    assert.fail('Expected final response.finished event.');
  }

  assert.equal(finished.finish.text, 'Hi');
  assert.deepEqual(finished.finish.reasoning, { text: 'why' });
  assert.equal(finished.finish.refusal, 'no');
  assert.equal(finished.finish.finishReason, 'tool_calls');
  assert.deepEqual(finished.finish.toolCalls, expectedToolCalls);
});

test('returns OpenRouter stream error events for malformed and provider errors', async () => {
  const malformed = createOpenRouterProvider({
    transport: fakeTransport({ streams: [['data: not-json\n\n']] }),
    apiKey: 'key',
  });
  const providerError = createOpenRouterProvider({
    transport: fakeTransport({ streams: [[sse({ error: { message: 'bad' } })]] }),
    apiKey: 'key',
  });

  assert.equal(
    (
      await collect(
        malformed.stream({
          model: 'openai/gpt-5',
          messages: [{ role: 'user', content: 'Hi' }],
        }),
      )
    ).at(-1)?.type,
    'error',
  );
  assert.equal(
    (
      await collect(
        providerError.stream({
          model: 'openai/gpt-5',
          messages: [{ role: 'user', content: 'Hi' }],
        }),
      )
    ).at(-1)?.type,
    'error',
  );
});

test('rejects blank OpenRouter API keys', async () => {
  const provider = createOpenRouterProvider({
    transport: fakeTransport({}),
    apiKey: ' ',
  });

  await assert.rejects(
    provider.complete({
      model: 'openai/gpt-5',
      messages: [{ role: 'user', content: 'Hi' }],
    }),
    (error: unknown) =>
      error instanceof ProviderErrorObject &&
      error.data.code === 'auth_missing',
  );
});

const sse = (value: unknown): string => `data: ${JSON.stringify(value)}\n\n`;
