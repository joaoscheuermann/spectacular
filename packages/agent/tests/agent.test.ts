import assert from 'node:assert/strict';
import test from 'node:test';

import {
  AgentErrorObject,
  createAgent,
  type AgentEvent,
} from '../src/index.js';
import { z } from 'zod';
import {
  call,
  collect,
  completeFinish,
  createProvider,
  createTools,
  streamEvents,
} from './fakes.js';
import type { ProviderFinished } from 'llms';
import { createMessageStorage } from 'messages';

test('complete stores user and final assistant messages and includes system in provider requests', async () => {
  const finish: ProviderFinished = {
    ...completeFinish('Final answer.'),
    usage: { inputTokens: 4, outputTokens: 2, totalTokens: 6 },
    reasoning: { summary: 'Short reasoning.' },
  };
  const fake = createProvider({
    complete: () => finish,
  });
  const tools = createTools({
    definitions: [{ name: 'lookup', inputSchema: { type: 'object' } }],
  });
  const messages = createMessageStorage();
  const agent = createAgent({
    provider: fake.provider,
    tools: tools.storage,
    messages,
    system: 'Follow instructions.',
    model: 'fake-model',
    temperature: 0.2,
    maxOutputTokens: 128,
    flags: { includeUsage: true },
  });

  const response = await agent.complete('Hello.');

  assert.equal(response.text, 'Final answer.');
  assert.equal(response.finishReason, 'stop');
  assert.deepEqual(response.usage, {
    inputTokens: 4,
    outputTokens: 2,
    totalTokens: 6,
  });
  assert.deepEqual(response.reasoning, { summary: 'Short reasoning.' });
  assert.equal(response.finish, finish);
  assert.deepEqual(fake.requests[0], {
    model: 'fake-model',
    messages: [
      { role: 'system', content: 'Follow instructions.' },
      { role: 'user', content: 'Hello.' },
    ],
    tools: [{ name: 'lookup', inputSchema: { type: 'object' } }],
    temperature: 0.2,
    maxOutputTokens: 128,
    flags: { includeUsage: true },
  });
  assert.deepEqual(messages.list(), [
    { role: 'user', content: 'Hello.' },
    { role: 'assistant', content: 'Final answer.' },
  ]);
});

test('complete executes requested tools and calls the provider again with tool results', async () => {
  const lookup = call('lookup', { query: 'doric' });
  const fake = createProvider({
    complete: (_request, index) =>
      index === 0
        ? completeFinish('Checking.', [lookup])
        : completeFinish('Tool says result.'),
  });
  const tools = createTools({ results: { lookup: { found: true } } });
  const messages = createMessageStorage();
  const agent = createAgent({
    provider: fake.provider,
    tools: tools.storage,
    messages,
    system: '',
    model: 'fake-model',
  });

  const response = await agent.complete('Find context.');

  assert.equal(response.text, 'Tool says result.');
  assert.equal(fake.requests.length, 2);
  assert.deepEqual(tools.calls, [
    { id: lookup.id, name: 'lookup', payload: { query: 'doric' } },
  ]);
  assert.deepEqual(messages.list(), [
    { role: 'user', content: 'Find context.' },
    {
      role: 'assistant',
      content: 'Checking.',
      toolCalls: [lookup],
    },
    {
      role: 'tool',
      toolCallId: lookup.id,
      content: '{"found":true}',
    },
    { role: 'assistant', content: 'Tool says result.' },
  ]);
  assert.deepEqual(fake.requests[1]?.messages, messages.list().slice(0, 3));
});

test('complete passes run structured output to provider requests and returns parsed output', async () => {
  const schema = z.object({ answer: z.string() });
  const finish: ProviderFinished<z.output<typeof schema>> = {
    ...completeFinish('{"answer":"Done"}'),
    structured: { answer: 'Done' },
  };
  const fake = createProvider({ complete: () => finish });
  const messages = createMessageStorage();
  const agent = createAgent({
    provider: fake.provider,
    tools: createTools().storage,
    messages,
    system: '',
    model: 'fake-model',
  });

  const response = await agent.complete('Return JSON.', { schema });
  const structured: { readonly answer: string } | undefined =
    response.structured;

  // @ts-expect-error structured output is inferred from the Zod schema.
  const invalid: number | undefined = response.structured;
  void invalid;

  assert.equal(fake.requests[0]?.schema, schema);
  assert.deepEqual(structured, { answer: 'Done' });
  assert.equal(response.finish, finish);
  assert.deepEqual(messages.list(), [
    { role: 'user', content: 'Return JSON.' },
    { role: 'assistant', content: '{"answer":"Done"}' },
  ]);
});

test('stream yields provider events tool events and final agent event across a tool loop', async () => {
  const lookup = call('lookup', { query: 'stream' });
  const fake = createProvider({
    stream: (_request, index) =>
      streamEvents(
        index === 0
          ? completeFinish('Need tool.', [lookup])
          : completeFinish('Stream done.'),
      ),
  });
  const tools = createTools({ results: { lookup: 'stream-result' } });
  const messages = createMessageStorage();
  const agent = createAgent({
    provider: fake.provider,
    tools: tools.storage,
    messages,
    system: 'Stream system.',
    model: 'fake-model',
  });

  const events = await collect(agent.stream('Stream input.'));

  assert.deepEqual(
    events.map((event) => event.type),
    [
      'agent.started',
      'response.started',
      'text.delta',
      'response.finished',
      'tool.started',
      'tool.finished',
      'response.started',
      'text.delta',
      'response.finished',
      'agent.finished',
    ],
  );
  assert.deepEqual(events[4], {
    type: 'tool.started',
    call: { id: lookup.id, name: 'lookup', payload: { query: 'stream' } },
  });
  assert.deepEqual(events[5], {
    type: 'tool.finished',
    call: { id: lookup.id, name: 'lookup', payload: { query: 'stream' } },
    result: 'stream-result',
    content: 'stream-result',
  });
  const final = events.at(-1);

  assert.equal(
    final?.type === 'agent.finished' ? final.response.text : undefined,
    'Stream done.',
  );
});

test('stream passes run structured output to provider requests and emits parsed output', async () => {
  const schema = z.object({ answer: z.string() });
  const finish: ProviderFinished<z.output<typeof schema>> = {
    ...completeFinish('{"answer":"Done"}'),
    structured: { answer: 'Done' },
  };
  const fake = createProvider({
    stream: () => streamEvents(finish),
  });
  const messages = createMessageStorage();
  const agent = createAgent({
    provider: fake.provider,
    tools: createTools().storage,
    messages,
    system: '',
    model: 'fake-model',
  });

  const events = await collect(agent.stream('Return JSON.', { schema }));
  const final = events.at(-1);

  assert.equal(fake.requests[0]?.schema, schema);
  assert.equal(final?.type, 'agent.finished');

  if (final?.type !== 'agent.finished') {
    assert.fail('Expected final agent.finished event.');
  }

  assert.deepEqual(final.response.structured, { answer: 'Done' });
  assert.deepEqual(messages.list(), [
    { role: 'user', content: 'Return JSON.' },
    { role: 'assistant', content: '{"answer":"Done"}' },
  ]);
});

test('does not persist the system prompt into external message storage', async () => {
  const fake = createProvider({ complete: () => completeFinish('Done.') });
  const messages = createMessageStorage();
  const agent = createAgent({
    provider: fake.provider,
    tools: createTools().storage,
    messages,
    system: 'Private system.',
    model: 'fake-model',
  });

  await agent.complete('Visible input.');

  assert.equal(fake.requests[0]?.messages[0]?.role, 'system');
  assert.deepEqual(messages.list(), [
    { role: 'user', content: 'Visible input.' },
    { role: 'assistant', content: 'Done.' },
  ]);
});

test('rejects concurrent runs with AgentErrorObject before storage mutation', async () => {
  let release!: (finish: ProviderFinished) => void;
  const pending = new Promise<ProviderFinished>((resolve) => {
    release = resolve;
  });
  const fake = createProvider({ complete: () => pending });
  const messages = createMessageStorage();
  const agent = createAgent({
    provider: fake.provider,
    tools: createTools().storage,
    messages,
    system: '',
    model: 'fake-model',
  });

  const first = agent.complete('First.');

  await assert.rejects(
    agent.complete('Second.'),
    (error: unknown) =>
      error instanceof AgentErrorObject && error.data.code === 'concurrent_run',
  );
  assert.deepEqual(messages.list(), [{ role: 'user', content: 'First.' }]);

  release(completeFinish('First done.'));
  await first;
});

test('stream yields tool.failed and propagates tool execution errors', async () => {
  const failure = new Error('tool failed');
  const lookup = call('lookup');
  const fake = createProvider({
    stream: () => streamEvents(completeFinish('Need tool.', [lookup])),
  });
  const agent = createAgent({
    provider: fake.provider,
    tools: createTools({ failure }).storage,
    messages: createMessageStorage(),
    system: '',
    model: 'fake-model',
  });
  const events: AgentEvent[] = [];

  await assert.rejects(
    async () => {
      for await (const event of agent.stream('Run tool.')) {
        events.push(event);
      }
    },
    (error: unknown) => error === failure,
  );

  assert.deepEqual(events.at(-1), {
    type: 'tool.failed',
    call: { id: lookup.id, name: 'lookup', payload: {} },
    error: failure,
  });
});

test('complete propagates provider errors unchanged', async () => {
  const failure = new Error('provider failed');
  const fake = createProvider({
    complete: () => {
      throw failure;
    },
  });
  const agent = createAgent({
    provider: fake.provider,
    tools: createTools().storage,
    messages: createMessageStorage(),
    system: '',
    model: 'fake-model',
  });

  await assert.rejects(
    agent.complete('Run provider.'),
    (error: unknown) => error === failure,
  );
});

test('stream propagates provider errors unchanged', async () => {
  const failure = new Error('provider stream failed');
  const fake = createProvider({
    stream: () => {
      throw failure;
    },
  });
  const agent = createAgent({
    provider: fake.provider,
    tools: createTools().storage,
    messages: createMessageStorage(),
    system: '',
    model: 'fake-model',
  });

  await assert.rejects(
    collect(agent.stream('Stream provider.')),
    (error: unknown) => error === failure,
  );
});

test('stream throws AgentErrorObject when the provider omits response.finished', async () => {
  const fake = createProvider({
    stream: () =>
      (async function* () {
        yield {
          type: 'text.delta',
          delta: 'unfinished',
        };
      })(),
  });
  const agent = createAgent({
    provider: fake.provider,
    tools: createTools().storage,
    messages: createMessageStorage(),
    system: '',
    model: 'fake-model',
  });

  await assert.rejects(
    collect(agent.stream('Unfinished.')),
    (error: unknown) =>
      error instanceof AgentErrorObject &&
      error.data.code === 'missing_provider_finish',
  );
});

test('serializes string object and undefined tool results into tool messages', async () => {
  const first = call('string');
  const second = call('object');
  const third = call('empty');
  const fake = createProvider({
    complete: (_request, index) =>
      index === 0
        ? completeFinish('Need tools.', [first, second, third])
        : completeFinish('Done.'),
  });
  const messages = createMessageStorage();
  const agent = createAgent({
    provider: fake.provider,
    tools: createTools({
      results: {
        string: 'plain text',
        object: { ok: true },
        empty: undefined,
      },
    }).storage,
    messages,
    system: '',
    model: 'fake-model',
  });

  await agent.complete('Serialize.');

  assert.deepEqual(
    messages.list().filter((message) => message.role === 'tool'),
    [
      { role: 'tool', toolCallId: first.id, content: 'plain text' },
      { role: 'tool', toolCallId: second.id, content: '{"ok":true}' },
      { role: 'tool', toolCallId: third.id, content: '' },
    ],
  );
});

test('throws AgentErrorObject when a tool result cannot be serialized', async () => {
  const cyclic: Record<string, unknown> = {};
  cyclic.self = cyclic;
  const cycle = call('cycle');
  const fake = createProvider({
    complete: () => completeFinish('Need tool.', [cycle]),
  });
  const messages = createMessageStorage();
  const agent = createAgent({
    provider: fake.provider,
    tools: createTools({ results: { cycle: cyclic } }).storage,
    messages,
    system: '',
    model: 'fake-model',
  });

  await assert.rejects(
    agent.complete('Serialize cycle.'),
    (error: unknown) =>
      error instanceof AgentErrorObject &&
      error.data.code === 'tool_result_serialization_failed',
  );
  assert.equal(
    messages.list().some((message) => message.role === 'tool'),
    false,
  );
});

test('continues tool loops until the provider returns a final response', async () => {
  const fake = createProvider({
    complete: (_request, index) =>
      index < 3
        ? completeFinish(`Tool turn ${index}.`, [call('repeat', { index })])
        : completeFinish('Final after tools.'),
  });
  const tools = createTools({ results: { repeat: 'again' } });
  const agent = createAgent({
    provider: fake.provider,
    tools: tools.storage,
    messages: createMessageStorage(),
    system: '',
    model: 'fake-model',
  });

  const response = await agent.complete('Loop.');

  assert.equal(response.text, 'Final after tools.');
  assert.equal(fake.requests.length, 4);
  assert.equal(tools.calls.length, 3);
});
