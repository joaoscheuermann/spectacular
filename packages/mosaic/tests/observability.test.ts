import assert from 'node:assert/strict';
import test from 'node:test';

import { mosaic, type MosaicEvent, type MosaicOptions } from '../src/index.js';
import type { LlmProvider, ProviderRequest } from 'llms';
import type { Tool } from 'tool';
import { z } from 'zod';

import { mosaicProviders, terminalFinish } from './structured.js';
import { createRuntime } from '../src/lib/observability.js';

test('observer delivery is awaited, serial, immutable, and ordered', async () => {
  const harness = createHarness();
  const events: MosaicEvent[] = [];
  const runId = '018f47d2-e3b1-7b4f-8b2c-1f5a7fdf1601';
  const controller = new AbortController();
  let active = false;

  const result = await mosaic(harness.options).prompt('private request', {
    runId,
    signal: controller.signal,
    capture: 'io',
    observer: async (event) => {
      assert.equal(active, false);
      active = true;
      await Promise.resolve();
      assert.throws(() => {
        (event as { sequence: number }).sequence = 0;
      }, TypeError);
      events.push(event);
      active = false;
    },
  });

  assert.equal(result.status, 'completed');
  assert.deepEqual(
    events.map(({ sequence }) => sequence),
    events.map((_, index) => index + 1),
  );
  assert.equal(events[0]?.type, 'run.started');
  assert.equal(events.at(-1)?.type, 'run.finished');
  assert.equal(
    events.every((event) => event.schemaVersion === 3),
    true,
  );
  assert.equal(
    events.every((event) => event.runId === runId),
    true,
  );
  assert.equal(
    events
      .filter(
        (event) =>
          event.type === 'model.request' ||
          event.type === 'model.response' ||
          event.type === 'structured.attempt' ||
          event.type === 'tool.repair',
      )
      .every((event) => event.providerId === 'fake'),
    true,
  );
  assert.equal(
    harness.requests.every((request) => request.signal === controller.signal),
    true,
  );
  assert.equal(
    events.filter(({ type }) => type === 'structured.attempt').length,
    3,
  );
  assert.equal(
    events.some(
      (event) =>
        event.type === 'model.request' &&
        JSON.stringify(event.content).includes('private request'),
    ),
    true,
  );
});

test('structure capture excludes prompts, parsed payloads, and model responses', async () => {
  const harness = createHarness();
  const events: MosaicEvent[] = [];

  await mosaic(harness.options).prompt('secret prompt', {
    observer: (event) => {
      events.push(event);
    },
  });

  const trace = JSON.stringify(events);
  assert.doesNotMatch(trace, /secret prompt/);
  for (const field of [
    'content',
    'input',
    'graph',
    'decision',
    'outcome',
    'delivery',
    'hints',
    'query',
    'output',
  ]) {
    assert.equal(
      events.some((event) => field in event),
      false,
      field,
    );
  }
});

test('observer failure aborts the run and prevents provider activity', async () => {
  const harness = createHarness();
  const failure = new Error('observer failed');
  let calls = 0;

  await assert.rejects(
    mosaic(harness.options).prompt('request', {
      observer: () => {
        calls += 1;
        throw failure;
      },
    }),
    (error: unknown) => error === failure,
  );
  assert.equal(calls, 1);
  assert.equal(harness.requests.length, 0);
});

test('undefined observer rejection still latches and aborts the run', async () => {
  const harness = createHarness();
  let resolved = false;
  let calls = 0;

  try {
    await mosaic(harness.options).prompt('request', {
      observer: () => {
        calls += 1;
        return Promise.reject(undefined);
      },
    });
    resolved = true;
  } catch (error) {
    assert.equal(error, undefined);
  }

  assert.equal(resolved, false);
  assert.equal(calls, 1);
  assert.equal(harness.requests.length, 0);
});

test('io capture deep-clones and freezes tool inputs and outputs', async () => {
  const lookup = tool('lookup');
  const harness = createHarness(lookup);
  const events: MosaicEvent[] = [];

  const result = await mosaic(harness.options).prompt('request', {
    capture: 'io',
    observer: (event) => {
      if (event.type === 'tool.started') {
        assert.throws(() => {
          (event.input as { query: string }).query = 'changed';
        }, TypeError);
      }
      if (event.type === 'tool.finished') {
        assert.throws(() => {
          (event.output as { found: boolean }).found = false;
        }, TypeError);
      }
      events.push(event);
    },
  });

  const started = events.find(({ type }) => type === 'tool.started');
  const finished = events.find(({ type }) => type === 'tool.finished');
  assert.deepEqual(started?.type === 'tool.started' ? started.input : null, {
    query: 'evidence',
  });
  assert.deepEqual(
    finished?.type === 'tool.finished' ? finished.output : null,
    { found: true },
  );
  assert.equal(
    finished?.type === 'tool.finished' ? finished.observationId : undefined,
    result.nodes[0]?.observations[0]?.id,
  );
});

test('io capture omits reasoning, usage, flags, auth, and request controls', async () => {
  const harness = createHarness();
  const contents: unknown[] = [];

  await mosaic(harness.options).prompt('request', {
    capture: 'io',
    observer: (event) => {
      if (
        (event.type === 'model.request' || event.type === 'model.response') &&
        event.content !== undefined
      ) {
        contents.push(event.content);
      }
    },
  });

  assert.ok(contents.length > 0);
  for (const key of [
    'reasoning',
    'usage',
    'flags',
    'auth',
    'authorization',
    'effort',
    'temperature',
    'maxOutputTokens',
  ]) {
    assert.equal(
      contents.some((content) => hasKey(content, key)),
      false,
      key,
    );
  }
});

test('io capture never exposes rejected structured candidates', async () => {
  const events: MosaicEvent[] = [];
  const runtime = createRuntime({
    capture: 'io',
    observer: (event) => {
      events.push(event);
    },
  });
  const provider = {
    metadata: { id: 'fake', name: 'Fake', baseUrl: 'https://fake.invalid' },
    complete: async () => ({
      text: 'private rejected text',
      finishReason: 'tool_calls' as const,
      structured: { private: 'rejected structured response' },
      toolCalls: [
        {
          id: 'terminal-call',
          name: 'submit_structured_output',
          arguments: '{"private":"rejected response"}',
        },
      ],
    }),
  } as unknown as LlmProvider;
  const observed = runtime.provider(provider, 'plan');

  await observed.complete({
    model: 'fake-model',
    messages: [
      {
        role: 'system',
        content: '# Structured output correction\n\nRetry safely.',
      },
      {
        role: 'assistant',
        content: 'private rejected request',
        toolCalls: [
          {
            id: 'previous-terminal-call',
            name: 'submit_structured_output',
            arguments: '{"private":"previous response"}',
          },
        ],
      },
    ],
    tools: [
      {
        name: 'submit_structured_output',
        description:
          'Submit the final structured output and end the agent run.',
        inputSchema: { type: 'object' },
        outputSchema: { not: {} },
      },
    ],
  });

  const trace = JSON.stringify(events);
  assert.doesNotMatch(
    trace,
    /private rejected|rejected response|previous response/u,
  );
  assert.match(trace, /submit_structured_output/u);
});

test('duration measurements exclude observer latency', async () => {
  const runtime = createRuntime({
    observer: async () => {
      await delay(40);
    },
  });
  const timer = runtime.timer();

  await runtime.emit({ type: 'stage.started', stage: 'plan' });

  assert.ok(runtime.duration(timer) < 25);
});

test('tool effects remain executed when observer fails on tool.finished', async () => {
  let effects = 0;
  const lookup = tool('lookup', async () => {
    effects += 1;
    return { found: true };
  });
  const harness = createHarness(lookup);
  const failure = new Error('terminal tool observer failed');
  const observed: string[] = [];

  await assert.rejects(
    mosaic(harness.options).prompt('request', {
      capture: 'io',
      observer: (event) => {
        observed.push(event.type);
        if (event.type === 'tool.finished') throw failure;
      },
    }),
    (error: unknown) => error === failure,
  );

  assert.equal(effects, 1);
  assert.equal(observed.at(-1), 'tool.finished');
});

test('node-bound bundle and execution events identify the active revision', async () => {
  const harness = createHarness(tool('lookup'));
  const events: MosaicEvent[] = [];

  await mosaic(harness.options).prompt('request', {
    observer: (event) => {
      events.push(event);
    },
  });

  const nodeEvents = events.filter(
    (event) =>
      'nodeId' in event &&
      event.nodeId === 'n01:finish' &&
      (event.stage === 'bundle' || event.stage === 'execution'),
  );
  assert.ok(nodeEvents.length > 0);
  assert.equal(
    nodeEvents.every((event) => 'revision' in event && event.revision === 1),
    true,
  );
  for (const type of [
    'retrieval.result',
    'bundle.selected',
    'menu.composed',
    'model.request',
    'model.response',
    'structured.attempt',
    'tool.started',
    'tool.finished',
    'decision.created',
    'observations.created',
    'outcome.created',
  ]) {
    assert.equal(
      nodeEvents.some((event) => event.type === type),
      true,
      type,
    );
  }
});

const plan = {
  nodes: [
    {
      id: 'n01:finish',
      goal: 'Produce the result.',
      doneWhen: ['The result is complete.'],
      dependsOn: [],
      deliver: true,
    },
  ],
};

const decision = {
  status: 'completed',
  criteria: [
    {
      criterionIndex: 0,
      satisfied: true,
      evidence: 'Result produced.',
      observationIds: [],
    },
  ],
  result: { markdown: 'Done.', artifacts: [] },
  revisionRequest: null,
  reason: null,
};

const createHarness = (lookup?: Tool) => {
  const requests: ProviderRequest<unknown>[] = [];
  let executionTurns = 0;
  const provider = {
    metadata: { id: 'fake', name: 'Fake', baseUrl: 'https://fake.invalid' },
    complete: async (request: ProviderRequest<unknown>) => {
      requests.push(request);
      const system = request.messages[0]?.content;
      const executing =
        typeof system === 'string' &&
        system.startsWith('You execute one outcome-oriented node');
      if (!executing) return privateFinish(request, plan);
      executionTurns += 1;
      if (lookup !== undefined && executionTurns === 1) {
        return {
          text: '',
          finishReason: 'tool_calls' as const,
          toolCalls: [
            {
              id: 'call-lookup',
              name: lookup.name,
              arguments: JSON.stringify({ query: 'evidence' }),
            },
          ],
        };
      }
      return privateFinish(request, decision);
    },
  } as unknown as LlmProvider;
  const options: MosaicOptions = {
    logger: { info: () => undefined, debug: () => undefined } as never,
    providers: mosaicProviders(provider),
    models: {
      planning: { model: 'default-model', effort: 'low' },
      revision: { model: 'default-model', effort: 'low' },
      execution: { model: 'default-model', effort: 'low' },
      reranker: 'reranker-model',
      embedder: 'embedder-model',
    },
    routing: {
      maxHintCandidates: 3,
      maxRetrievedCandidates: 3,
      maxSkills: 3,
    },
    execution: { maxTurns: 4 },
    revision: { max: 1 },
    skills: {
      required: [],
      menu: [],
      retriever: { search: async () => [] },
    },
    tools: {
      required: lookup === undefined ? [] : [lookup],
      menu: lookup === undefined ? [] : [lookup],
      retriever: { search: async () => [] },
    },
  };

  return { options, requests };
};

const tool = (
  name: string,
  execute: Tool['execute'] = async () => ({ found: true }),
): Tool => {
  const input = z.object({ query: z.string() });
  const output = z.object({ found: z.boolean() });
  return {
    name,
    description: `${name} tool`,
    input,
    output,
    definition: {
      name,
      description: `${name} tool`,
      inputSchema: {
        type: 'object',
        properties: { query: { type: 'string' } },
        required: ['query'],
        additionalProperties: false,
      },
      outputSchema: {
        type: 'object',
        properties: { found: { type: 'boolean' } },
        required: ['found'],
        additionalProperties: false,
      },
      strict: true,
    },
    execute,
  };
};

const privateFinish = (request: ProviderRequest<unknown>, value: unknown) => ({
  ...terminalFinish(request, value),
  reasoning: { text: 'private chain of thought', effort: 'high' as const },
  usage: { inputTokens: 100, outputTokens: 50, reasoningTokens: 25 },
});

const hasKey = (value: unknown, key: string): boolean => {
  if (typeof value !== 'object' || value === null) return false;
  if (Object.prototype.hasOwnProperty.call(value, key)) return true;
  return Object.values(value).some((nested) => hasKey(nested, key));
};

const delay = (milliseconds: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, milliseconds));
