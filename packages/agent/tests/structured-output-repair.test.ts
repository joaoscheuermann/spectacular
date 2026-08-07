import assert from 'node:assert/strict';
import test from 'node:test';

import { AgentErrorObject, createAgent, type Agent } from '../src/index.js';
import {
  ProviderErrorObject,
  type ProviderFinished,
  type ProviderRequest,
} from 'llms';
import { createMessageStorage } from 'messages';
import type { ToolCallRequest } from 'tool';
import { z } from 'zod';

import {
  call,
  collect,
  completeFinish,
  createProvider,
  createTools,
  streamEvents,
} from './fakes.js';

const terminalDescription =
  'Submit the final structured output and end the agent run.';
const answerSchema = z.object({ answer: z.string() });
const lookupDefinition = {
  name: 'lookup',
  inputSchema: { type: 'object' as const },
};

type Mode = 'complete' | 'stream';

type InvalidSubmission = {
  readonly name: string;
  readonly reason: RegExp;
  readonly finish: (request: ProviderRequest<unknown>) => ProviderFinished;
};

const terminalName = (request: ProviderRequest<unknown>): string => {
  const terminal = request.tools?.find(
    ({ description }) => description === terminalDescription,
  );

  assert.ok(terminal);
  return terminal.name;
};

const malformedCall = (name: string): ToolCallRequest => ({
  id: 'call_malformed',
  name,
  arguments: '{"answer":',
});

const correctionFrom = (
  request: ProviderRequest<unknown> | undefined,
): string | undefined =>
  request?.messages
    .filter(({ role }) => role === 'system')
    .map(({ content }) => (typeof content === 'string' ? content : ''))
    .find((content) => content.startsWith('# Structured output correction'));

const invoke = async (agent: Agent, mode: Mode) => {
  if (mode === 'complete') {
    return agent.complete('Return evidence.', { schema: answerSchema });
  }

  const events = await collect(
    agent.stream('Return evidence.', { schema: answerSchema }),
  );
  const final = events.at(-1);

  assert.equal(final?.type, 'agent.finished');
  if (final?.type !== 'agent.finished') {
    assert.fail('Expected the stream to finish successfully.');
  }

  return final.response;
};

const invalidSubmissions: readonly InvalidSubmission[] = [
  {
    name: 'a missing terminal call',
    reason: /ended without terminal structured output/i,
    finish: () => completeFinish('Unstructured answer.'),
  },
  {
    name: 'malformed terminal arguments',
    reason: /arguments must be valid JSON/i,
    finish: (request) =>
      completeFinish('', [malformedCall(terminalName(request))]),
  },
  {
    name: 'terminal arguments that fail the schema',
    reason: /failed schema validation/i,
    finish: (request) =>
      completeFinish('', [call(terminalName(request), { answer: 42 })]),
  },
  {
    name: 'duplicate terminal calls',
    reason: /must be the only tool call/i,
    finish: (request) => {
      const name = terminalName(request);

      return completeFinish('', [
        call(name, { answer: 'First' }, 'call_terminal_1'),
        call(name, { answer: 'Second' }, 'call_terminal_2'),
      ]);
    },
  },
  {
    name: 'mixed terminal and executable calls',
    reason: /must be the only tool call/i,
    finish: (request) =>
      completeFinish('', [
        call('lookup', { query: 'must-not-run' }),
        call(terminalName(request), { answer: 'Done' }),
      ]),
  },
];

for (const mode of ['complete', 'stream'] as const) {
  for (const submission of invalidSubmissions) {
    test(`${mode} repairs ${submission.name} without storing or executing it`, async () => {
      const fake = createProvider({
        complete: (request, index) =>
          index === 0
            ? submission.finish(request)
            : completeFinish('', [
                call(terminalName(request), { answer: 'Done' }),
              ]),
        stream: (request, index) =>
          streamEvents(
            index === 0
              ? submission.finish(request)
              : completeFinish('', [
                  call(terminalName(request), { answer: 'Done' }),
                ]),
          ),
      });
      const tools = createTools({ definitions: [lookupDefinition] });
      const messages = createMessageStorage();
      const agent = createAgent({
        provider: fake.provider,
        tools: tools.storage,
        messages,
        system: '',
        model: 'fake-model',
      });

      const response = await invoke(agent, mode);

      assert.deepEqual(response.structured, { answer: 'Done' });
      assert.equal(fake.requests.length, 2);
      assert.equal(correctionFrom(fake.requests[0]), undefined);
      const correction = correctionFrom(fake.requests[1]);
      assert.ok(correction);
      assert.match(correction, new RegExp(terminalName(fake.requests[1])));
      assert.match(correction, submission.reason);
      assert.deepEqual(tools.calls, []);
      assert.deepEqual(messages.list(), [
        { role: 'user', content: 'Return evidence.' },
        { role: 'assistant', content: '{"answer":"Done"}' },
      ]);
    });
  }
}

test('correction feedback includes at most ten normalized issues without rejected values', async () => {
  const schema = z.object({
    items: z.array(z.object({ count: z.number() })),
  });
  const rejected = {
    items: Array.from({ length: 12 }, (_, index) => ({
      count: `private-value-${index}`,
    })),
  };
  const accepted = {
    items: Array.from({ length: 12 }, (_, index) => ({ count: index })),
  };
  const fake = createProvider({
    complete: (request, index) =>
      completeFinish('', [
        call(terminalName(request), index === 0 ? rejected : accepted),
      ]),
  });
  const agent = createAgent({
    provider: fake.provider,
    tools: createTools({ definitions: [lookupDefinition] }).storage,
    messages: createMessageStorage(),
    system: '',
    model: 'fake-model',
  });

  await agent.complete('Return counts.', { schema });

  const correction = correctionFrom(fake.requests[1]);
  assert.ok(correction);
  assert.match(correction, /items\.0\.count:/);
  assert.match(correction, /items\.9\.count:/);
  assert.doesNotMatch(correction, /items\.10\.count:/);
  assert.doesNotMatch(correction, /private-value/);
});

for (const mode of ['complete', 'stream'] as const) {
  test(`${mode} throws the fourth invalid submission with its latest diagnostic`, async () => {
    const fake = createProvider({
      complete: (request, index) => exhaustionFinish(request, index),
      stream: (request, index) =>
        streamEvents(exhaustionFinish(request, index)),
    });
    const messages = createMessageStorage();
    const agent = createAgent({
      provider: fake.provider,
      tools: createTools({ definitions: [lookupDefinition] }).storage,
      messages,
      system: '',
      model: 'fake-model',
    });
    let caught: unknown;

    try {
      await invoke(agent, mode);
    } catch (error) {
      caught = error;
    }

    assert.ok(caught instanceof AgentErrorObject);
    assert.equal(caught.data.code, 'invalid_structured_output');
    assert.match(caught.data.message, /failed schema validation/i);
    assert.match(caught.data.diagnostic ?? '', /answer:/);
    assert.equal(fake.requests.length, 4);
    assert.equal(correctionFrom(fake.requests[0]), undefined);
    assert.ok(correctionFrom(fake.requests[1]));
    assert.ok(correctionFrom(fake.requests[2]));
    assert.ok(correctionFrom(fake.requests[3]));
    assert.deepEqual(messages.list(), [
      { role: 'user', content: 'Return evidence.' },
    ]);
  });
}

test('ordinary tool turns consume transient corrections without changing the retry budget', async () => {
  const sequence = ['first', 'second', 'third'] as const;
  const fake = createProvider({
    complete: (request, index) => {
      if (index === 0) return completeFinish('Missing terminal.');
      if (index === 1) return completeFinish('', [call(sequence[0])]);
      if (index === 2) {
        return completeFinish('', [malformedCall(terminalName(request))]);
      }
      if (index === 3) return completeFinish('', [call(sequence[1])]);
      if (index === 4) {
        return completeFinish('', [
          call(terminalName(request), { answer: 42 }),
        ]);
      }
      if (index === 5) return completeFinish('', [call(sequence[2])]);
      if (index === 6) {
        return completeFinish('', [
          call('lookup', { query: 'must-not-run' }),
          call(terminalName(request), { answer: 'Done' }),
        ]);
      }

      throw new Error('Retry budget was reset by an ordinary tool turn.');
    },
  });
  const tools = createTools({
    definitions: [
      lookupDefinition,
      ...sequence.map((name) => ({
        name,
        inputSchema: { type: 'object' as const },
      })),
    ],
  });
  const messages = createMessageStorage();
  const agent = createAgent({
    provider: fake.provider,
    tools: tools.storage,
    messages,
    system: '',
    model: 'fake-model',
  });

  await assert.rejects(
    agent.complete('Use tools and return evidence.', { schema: answerSchema }),
    (error: unknown) =>
      error instanceof AgentErrorObject &&
      error.data.code === 'invalid_structured_output',
  );

  assert.equal(fake.requests.length, 7);
  assert.deepEqual(
    fake.requests.map((request) => correctionFrom(request) !== undefined),
    [false, true, false, true, false, true, false],
  );
  assert.deepEqual(
    tools.calls.map(({ name }) => name),
    [...sequence],
  );
  assert.deepEqual(
    messages
      .list()
      .filter(({ role }) => role === 'assistant')
      .flatMap(({ toolCalls }) => toolCalls?.map(({ name }) => name) ?? []),
    [...sequence],
  );
});

test('stream preserves invalid deltas while suppressing the invalid finished event', async () => {
  const fake = createProvider({
    stream: (request, index) =>
      streamEvents(
        index === 0
          ? {
              ...completeFinish('visible rejected delta', [
                call(terminalName(request), { answer: 42 }),
              ]),
            }
          : completeFinish('', [
              call(terminalName(request), { answer: 'Done' }),
            ]),
      ),
  });
  const messages = createMessageStorage();
  const agent = createAgent({
    provider: fake.provider,
    tools: createTools({ definitions: [lookupDefinition] }).storage,
    messages,
    system: '',
    model: 'fake-model',
  });

  const events = await collect(
    agent.stream('Return evidence.', { schema: answerSchema }),
  );

  assert.equal(
    events.filter(({ type }) => type === 'response.finished').length,
    1,
  );
  assert.equal(
    events.filter(({ type }) => type === 'agent.finished').length,
    1,
  );
  assert.equal(
    events.some(
      (event) =>
        event.type === 'text.delta' && event.delta === 'visible rejected delta',
    ),
    true,
  );
  assert.deepEqual(messages.list(), [
    { role: 'user', content: 'Return evidence.' },
    { role: 'assistant', content: '{"answer":"Done"}' },
  ]);
});

for (const mode of ['complete', 'stream'] as const) {
  test(`${mode} leaves provider-native structured failures unchanged`, async () => {
    const failure = new ProviderErrorObject({
      provider: 'fake',
      code: 'invalid_structured_output',
      message: 'Fake provider rejected native structured output.',
    });
    const fake = createProvider({
      complete: () => {
        throw failure;
      },
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
    const operation =
      mode === 'complete'
        ? agent.complete('Return evidence.', { schema: answerSchema })
        : collect(agent.stream('Return evidence.', { schema: answerSchema }));

    await assert.rejects(operation, (error: unknown) => error === failure);
    assert.equal(fake.requests.length, 1);
    assert.equal(fake.requests[0]?.schema, answerSchema);
  });
}

const exhaustionFinish = (
  request: ProviderRequest<unknown>,
  index: number,
): ProviderFinished => {
  if (index === 0) return completeFinish('Missing terminal.');
  if (index === 1) {
    return completeFinish('', [malformedCall(terminalName(request))]);
  }
  if (index === 2) {
    const name = terminalName(request);

    return completeFinish('', [
      call(name, { answer: 'First' }, 'call_terminal_1'),
      call(name, { answer: 'Second' }, 'call_terminal_2'),
    ]);
  }

  return completeFinish('', [call(terminalName(request), { answer: index })]);
};
