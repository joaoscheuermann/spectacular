import assert from 'node:assert/strict';
import test from 'node:test';

import { z } from 'zod';

import {
  ProviderErrorObject,
  createOpenAiProvider,
  type LlmDebugRecord,
} from '../src/index.js';
import { collect, fakeTransport, response } from './fakes.js';

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

test('returns parsed OpenAI structured output from content part stream snapshots', async () => {
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

  assert.deepEqual(finished.finish.structured, { answer: 'Done' });
});

test('returns parsed OpenAI structured output from snapshots when final output text is empty', async () => {
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

  assert.deepEqual(finished.finish.structured, { answer: 'Snapshot' });
});

test('prefers OpenAI text deltas over stream snapshots', async () => {
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

  assert.deepEqual(finished.finish.structured, { answer: 'Delta' });
});

test('returns parsed OpenAI structured output from output item stream snapshots', async () => {
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

  assert.deepEqual(finished.finish.structured, { answer: 'Done' });
});

test('returns parsed OpenAI structured output from text done stream snapshots', async () => {
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

test('logs OpenAI stream structured output failures with finish context', async () => {
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

  await assert.rejects(
    collect(
      provider.stream({
        model: 'gpt-5',
        messages: [{ role: 'user', content: 'Hi' }],
        schema: z.object({ answer: z.string() }),
      }),
    ),
    (error: unknown) =>
      error instanceof ProviderErrorObject &&
      error.data.code === 'invalid_structured_output',
  );

  assert.ok(records.some((record) => record.event === 'http.request'));
  assert.ok(
    records.some((record) => record.event === 'stream.response.completed'),
  );

  const finishRecord = records.find(
    (record) => record.event === 'response.finish',
  );
  const errorRecord = records.find(
    (record) => record.event === 'structured_output.error',
  );
  const finishFields = finishRecord?.fields as Record<string, unknown>;
  const errorFields = errorRecord?.fields as Record<string, unknown>;
  const finish = errorFields.finish as Record<string, unknown>;
  const error = errorFields.error as Record<string, unknown>;
  const cause = error.cause as Record<string, unknown>;

  assert.equal(finishFields.source, 'stream');
  assert.equal(errorFields.source, 'stream');
  assert.equal(finish.textLength, 0);
  assert.equal(finish.textExcerpt, '');
  assert.equal(error.code, 'invalid_structured_output');
  assert.match(String(cause.message), /JSON/);
});

const sse = (value: unknown): string => `data: ${JSON.stringify(value)}\n\n`;
