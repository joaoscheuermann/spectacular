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
import { createToolStorage, defineTool } from 'tool';

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
    definitions: [
      { name: 'lookup', inputSchema: { type: 'object' }, outputSchema: {} },
    ],
  });
  const messages = createMessageStorage();
  const agent = createAgent({
    provider: fake.provider,
    tools: tools.storage,
    messages,
    system: 'Follow instructions.',
    model: 'fake-model',
    effort: 'high',
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
    tools: [
      { name: 'lookup', inputSchema: { type: 'object' }, outputSchema: {} },
    ],
    effort: 'high',
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

test('rejects an invalid tool-call batch before any handler executes', async () => {
  let executions = 0;
  const tools = createToolStorage(
    ['first', 'second'].map((name) =>
      defineTool({
        name,
        input: z.object({ value: z.string() }),
        output: z.string(),
        execute: (_sandbox, { value }) => {
          executions += 1;
          return value;
        },
      })(undefined as never),
    ),
  );
  const repairs: number[] = [];
  const fake = createProvider({
    complete: (_request, index) =>
      index === 0
        ? completeFinish('', [
            call('first', { value: 'valid' }),
            call('second', { value: 42 }),
          ])
        : completeFinish('Recovered.'),
  });
  const agent = createAgent({
    provider: fake.provider,
    tools,
    messages: createMessageStorage(),
    system: '',
    model: 'fake-model',
  });

  const response = await agent.complete('Run tools.', {
    onToolCallRepair: ({ attempt }) => {
      repairs.push(attempt);
    },
  });

  assert.equal(response.text, 'Recovered.');
  assert.equal(executions, 0);
  assert.deepEqual(repairs, [1]);
  assert.equal(
    fake.requests[1]?.messages.filter(
      ({ role, toolResultStatus }) =>
        role === 'tool' && toolResultStatus === 'incomplete',
    ).length,
    2,
  );
});

test('complete uses a terminal tool for structured output with empty tool storage', async () => {
  const schema = z.object({ answer: z.string() });
  const fake = createProvider({
    complete: (request) => {
      const terminal = request.tools?.[0];

      assert.equal(request.tools?.length, 1);
      assert.equal(terminal?.name, 'submit_structured_output');
      return completeFinish('', [
        call(terminal?.name ?? '', { answer: 'Done' }),
      ]);
    },
  });
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

  assert.equal(fake.requests[0]?.schema, undefined);
  assert.deepEqual(structured, { answer: 'Done' });
  assert.deepEqual(response.finish.toolCalls, []);
  assert.deepEqual(messages.list(), [
    { role: 'user', content: 'Return JSON.' },
    { role: 'assistant', content: '{"answer":"Done"}' },
  ]);
});

test('complete uses a terminal tool for structured output after executable tools', async () => {
  const schema = z.object({ answer: z.string() });
  const lookup = call('lookup', { query: 'doric' });
  const fake = createProvider({
    complete: (request, index) => {
      if (index === 0) return completeFinish('', [lookup]);

      const terminal = request.tools?.find(
        ({ description }) =>
          description ===
          'Submit the final structured output and end the agent run.',
      );

      assert.ok(terminal);
      return completeFinish('', [call(terminal.name, { answer: 'Done' })]);
    },
  });
  const tools = createTools({
    definitions: [
      {
        name: 'lookup',
        inputSchema: { type: 'object' },
        outputSchema: {},
        strict: true,
      },
      {
        name: 'submit_structured_output',
        inputSchema: { type: 'object' },
        outputSchema: { not: {} },
      },
    ],
    results: { lookup: { found: true } },
  });
  const messages = createMessageStorage();
  const agent = createAgent({
    provider: fake.provider,
    tools: tools.storage,
    messages,
    system: 'Use evidence.',
    model: 'fake-model',
  });

  const response = await agent.complete('Return evidence.', { schema });

  assert.deepEqual(response.structured, { answer: 'Done' });
  assert.equal(response.finish.toolCalls.length, 0);
  assert.equal(fake.requests.length, 2);
  assert.equal(fake.requests[0]?.schema, undefined);
  assert.equal(fake.requests[1]?.schema, undefined);
  assert.equal(fake.requests[0]?.tools?.length, 3);
  assert.equal(fake.requests[0]?.toolChoice, undefined);
  assert.equal(fake.requests[0]?.parallelToolCalls, false);
  assert.equal(
    fake.requests[0]?.tools?.find(
      ({ description }) =>
        description ===
        'Submit the final structured output and end the agent run.',
    )?.name,
    'submit_structured_output_2',
  );
  assert.deepEqual(tools.calls, [
    { id: lookup.id, name: 'lookup', payload: { query: 'doric' } },
  ]);
});

test('complete rejects terminal tool arguments that fail the output schema', async () => {
  const schema = z.object({ answer: z.string() });
  const fake = createProvider({
    complete: (request) => {
      const terminal = request.tools?.find(
        ({ description }) =>
          description ===
          'Submit the final structured output and end the agent run.',
      );

      assert.ok(terminal);
      return completeFinish('', [call(terminal.name, { answer: 42 })]);
    },
  });
  const agent = createAgent({
    provider: fake.provider,
    tools: createTools({
      definitions: [
        { name: 'lookup', inputSchema: { type: 'object' }, outputSchema: {} },
      ],
    }).storage,
    messages: createMessageStorage(),
    system: '',
    model: 'fake-model',
  });

  await assert.rejects(
    agent.complete('Return evidence.', { schema }),
    (error: unknown) =>
      error instanceof AgentErrorObject &&
      error.data.code === 'invalid_structured_output',
  );
});

test('complete rejects a text finish when a terminal structured output is required', async () => {
  const agent = createAgent({
    provider: createProvider({
      complete: () => completeFinish('Unstructured answer.'),
    }).provider,
    tools: createTools({
      definitions: [
        { name: 'lookup', inputSchema: { type: 'object' }, outputSchema: {} },
      ],
    }).storage,
    messages: createMessageStorage(),
    system: '',
    model: 'fake-model',
  });

  await assert.rejects(
    agent.complete('Return evidence.', {
      schema: z.object({ answer: z.string() }),
    }),
    (error: unknown) =>
      error instanceof AgentErrorObject &&
      error.data.code === 'invalid_structured_output',
  );
});

test('complete rejects terminal structured output mixed with executable calls', async () => {
  const fake = createProvider({
    complete: (request) => {
      const terminal = request.tools?.find(
        ({ description }) =>
          description ===
          'Submit the final structured output and end the agent run.',
      );

      assert.ok(terminal);
      return completeFinish('', [
        call('lookup'),
        call(terminal.name, { answer: 'Done' }),
      ]);
    },
  });
  const tools = createTools({
    definitions: [
      { name: 'lookup', inputSchema: { type: 'object' }, outputSchema: {} },
    ],
  });
  const agent = createAgent({
    provider: fake.provider,
    tools: tools.storage,
    messages: createMessageStorage(),
    system: '',
    model: 'fake-model',
  });

  await assert.rejects(
    agent.complete('Return evidence.', {
      schema: z.object({ answer: z.string() }),
    }),
    (error: unknown) =>
      error instanceof AgentErrorObject &&
      error.data.code === 'invalid_structured_output',
  );
  assert.deepEqual(tools.calls, []);
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

test('stream uses a terminal tool for structured output with empty tool storage', async () => {
  const schema = z.object({ answer: z.string() });
  const fake = createProvider({
    stream: (request) => {
      const terminal = request.tools?.[0];

      assert.equal(request.tools?.length, 1);
      assert.equal(terminal?.name, 'submit_structured_output');
      return streamEvents(
        completeFinish('', [call(terminal?.name ?? '', { answer: 'Done' })]),
      );
    },
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

  assert.equal(fake.requests[0]?.schema, undefined);
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

test('stream uses a terminal tool for structured output after executable tools', async () => {
  const schema = z.object({ answer: z.string() });
  const lookup = call('lookup', { query: 'stream' });
  const fake = createProvider({
    stream: (request, index) => {
      if (index === 0) return streamEvents(completeFinish('', [lookup]));

      const terminal = request.tools?.find(
        ({ description }) =>
          description ===
          'Submit the final structured output and end the agent run.',
      );

      assert.ok(terminal);
      return streamEvents(
        completeFinish('', [call(terminal.name, { answer: 'Done' })]),
      );
    },
  });
  const tools = createTools({
    definitions: [
      { name: 'lookup', inputSchema: { type: 'object' }, outputSchema: {} },
    ],
    results: { lookup: 'evidence' },
  });
  const agent = createAgent({
    provider: fake.provider,
    tools: tools.storage,
    messages: createMessageStorage(),
    system: '',
    model: 'fake-model',
  });

  const events = await collect(agent.stream('Return evidence.', { schema }));
  const final = events.at(-1);

  assert.equal(final?.type, 'agent.finished');
  if (final?.type !== 'agent.finished') return;
  assert.deepEqual(final.response.structured, { answer: 'Done' });
  assert.equal(final.response.finish.toolCalls.length, 0);
  assert.equal(fake.requests[0]?.schema, undefined);
  assert.equal(fake.requests[1]?.schema, undefined);
  assert.deepEqual(tools.calls, [
    { id: lookup.id, name: 'lookup', payload: { query: 'stream' } },
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
