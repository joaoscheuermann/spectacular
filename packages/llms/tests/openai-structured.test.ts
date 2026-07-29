import assert from 'node:assert/strict';
import test from 'node:test';

import { z } from 'zod';

import {
  ProviderErrorObject,
  createOpenAiProvider,
  type LlmDebugRecord,
} from '../src/index.js';
import { collect, fakeTransport, response } from './fakes.js';

test('allows OpenAI structured output from stream finishes without parsing', async () => {
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

  assert.equal(finished.finish.structured, undefined);
});

test('allows OpenAI content part stream snapshots without structured parsing', async () => {
  const provider = createOpenAiProvider({
    transport: fakeTransport({
      streams: [
        [
          sse({
            type: 'response.content_part.done',
            part: { type: 'output_text', text: '{"answer":"Done"}' },
          }),
          sse({
            type: 'response.completed',
            response: { status: 'completed' },
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

  assert.equal(finished.finish.structured, undefined);
});

test('allows OpenAI stream snapshots when final output text is empty', async () => {
  const provider = createOpenAiProvider({
    transport: fakeTransport({
      streams: [
        [
          sse({
            type: 'response.content_part.done',
            part: { type: 'output_text', text: '{"answer":"Snapshot"}' },
          }),
          sse({
            type: 'response.completed',
            response: { status: 'completed', output_text: '' },
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

  assert.equal(finished.finish.structured, undefined);
});

test('prefers OpenAI text deltas over stream snapshots without structured parsing', async () => {
  const provider = createOpenAiProvider({
    transport: fakeTransport({
      streams: [
        [
          sse({
            type: 'response.output_text.delta',
            delta: '{"answer":"Delta"}',
          }),
          sse({
            type: 'response.content_part.done',
            part: { type: 'output_text', text: '{"answer":"Snapshot"}' },
          }),
          sse({
            type: 'response.completed',
            response: {
              status: 'completed',
              output_text: '{"answer":"Completed"}',
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

  assert.equal(finished.finish.structured, undefined);
});

test('allows OpenAI output item stream snapshots without structured parsing', async () => {
  const provider = createOpenAiProvider({
    transport: fakeTransport({
      streams: [
        [
          sse({
            type: 'response.output_item.done',
            output_index: 0,
            item: {
              type: 'message',
              content: [{ type: 'output_text', text: '{"answer":"Done"}' }],
            },
          }),
          sse({
            type: 'response.completed',
            response: { status: 'completed' },
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

  assert.equal(finished.finish.structured, undefined);
});

test('allows OpenAI text done stream snapshots without structured parsing', async () => {
  const provider = createOpenAiProvider({
    transport: fakeTransport({
      streams: [
        [
          sse({
            type: 'response.output_text.done',
            text: '{"answer":"Done"}',
          }),
          sse({
            type: 'response.completed',
            response: { status: 'completed' },
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

  assert.equal(finished.finish.structured, undefined);
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

test('allows invalid OpenAI structured JSON from streams', async () => {
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
  const events = await collect(streamProvider.stream(request));
  const finished = events.at(-1);

  assert.equal(finished?.type, 'response.finished');

  if (finished?.type !== 'response.finished') {
    assert.fail('Expected final response.finished event.');
  }

  assert.equal(finished.finish.text, 'not-json');
  assert.equal(finished.finish.structured, undefined);
});

test('does not log OpenAI stream structured output failures', async () => {
  const records: LlmDebugRecord[] = [];
  const provider = createOpenAiProvider({
    transport: fakeTransport({
      streams: [
        [
          sse({
            type: 'response.completed',
            response: {
              status: 'completed',
              output_text: '',
              output: [],
            },
          }),
        ],
      ],
    }),
    apiKey: 'sk-testSecret123',
    debugLogger: {
      async log(record): Promise<void> {
        records.push(record);
      },
    },
  });

  const events = await collect(
    provider.stream({
      model: 'gpt-5',
      messages: [{ role: 'user', content: 'Hi' }],
      schema: z.object({ answer: z.string() }),
    }),
  );
  const finished = events.at(-1);

  assert.ok(records.some((record) => record.event === 'http.request'));
  assert.ok(
    records.some((record) => record.event === 'stream.response.completed'),
  );

  assert.equal(finished?.type, 'response.finished');

  if (finished?.type !== 'response.finished') {
    assert.fail('Expected final response.finished event.');
  }

  assert.equal(finished.finish.structured, undefined);
  assert.equal(
    records.some((record) => record.event === 'structured_output.error'),
    false,
  );
});

test('omits sensitive structured output from errors and debug records', async () => {
  const sentinel = 'RATIONALE_DEBUG_PRIVATE';
  const records: LlmDebugRecord[] = [];
  const provider = createOpenAiProvider({
    transport: fakeTransport({
      responses: [
        response({
          status: 'completed',
          output_text: JSON.stringify({ reasoning: sentinel }),
          output: [],
        }),
      ],
    }),
    apiKey: 'sk-testSecret123',
    debugLogger: {
      async log(record): Promise<void> {
        records.push(record);
      },
    },
  });

  let caught: unknown;
  try {
    await provider.complete({
      model: 'gpt-5',
      messages: [{ role: 'user', content: 'Hi' }],
      schema: z.object({ answer: z.string() }),
      flags: { sensitiveOutput: true },
    });
  } catch (error) {
    caught = error;
  }

  assert.ok(caught instanceof ProviderErrorObject);
  assert.equal(caught.data.code, 'invalid_structured_output');
  assert.equal(caught.data.diagnostic, undefined);
  assert.equal(
    (caught as Error & { readonly cause?: unknown }).cause,
    undefined,
  );
  assert.doesNotMatch(JSON.stringify(records), new RegExp(sentinel, 'u'));
  assert.doesNotMatch(
    JSON.stringify({
      message: caught.message,
      data: caught.data,
      cause: (caught as Error & { readonly cause?: unknown }).cause,
    }),
    new RegExp(sentinel, 'u'),
  );
  const finish = records.find((record) => record.event === 'response.finish')
    ?.fields?.finish as Record<string, unknown>;
  assert.equal(finish.textExcerpt, undefined);
});

const sse = (value: unknown): string => `data: ${JSON.stringify(value)}\n\n`;
