import assert from 'node:assert/strict';
import test from 'node:test';

import type { Skill } from 'bundle';
import type { LlmProvider, ProviderFinished, ProviderRequest } from 'llms';
import { z } from 'zod';
import type { Tool } from 'tool';

import { execution } from '../src/lib/states/execution/index.js';
import type { Graph, Node } from '../src/lib/types/graph.js';
import type {
  WorkflowContext,
  WorkflowState,
} from '../src/lib/types/workflow.js';

test('fails without executing workflow work when no graph exists', async () => {
  const action = await execution(
    state([]),
    {} as never,
    {
      fail: (error: unknown) => ({ type: 'fail' as const, error }),
    } as never,
  );

  assert.equal(action.type, 'fail');
  if (action.type !== 'fail') return;
  assert.equal(
    (action.error as Error).message,
    'Impossible to continue, missing active graph!',
  );
});

test('completes a node stores result artifacts and schedules the next wave', async () => {
  const node = createNode('current');
  const graph: Graph = { revision: 1, nodes: [node] };
  const provider = createProvider([
    terminalFinish(
      completed({
        result: {
          markdown: ' \n## Final result\n',
          artifacts: [{ kind: 'inline', mime: 'text/plain', data: 'extra' }],
        },
      }),
    ),
  ]);
  const harness = createHarness(graph, provider);

  const action = await execution(state([graph]), harness.context, handlers());

  assert.deepEqual(action, {
    type: 'transition',
    handler: 'schedule',
    state: state([graph]),
  });
  assert.equal(node.status, 'completed');
  assert.deepEqual(node.artifacts, [
    {
      kind: 'inline',
      mime: 'text/markdown',
      data: ' \n## Final result\n',
    },
    { kind: 'inline', mime: 'text/plain', data: 'extra' },
  ]);
  assert.equal(provider.requests.length, 1);
  assert.equal(provider.requests[0]?.schema, undefined);
  assert.equal(provider.requests[0]?.tools?.length, 1);
  assert.equal(provider.requests[0]?.messages[0]?.role, 'system');
  assert.ok(provider.requests[0]?.messages.some(({ role }) => role === 'user'));
});

test('automatically records one returned tool observation for a completed node', async () => {
  const node = createNode('current', ['lookup']);
  const graph: Graph = { revision: 1, nodes: [node] };
  const provider = createProvider([
    toolFinish('call-1', 'lookup'),
    terminalFinish(completed()),
  ]);
  const calls: unknown[] = [];
  const harness = createHarness(graph, provider, [
    tool('lookup', async (payload) => {
      calls.push(payload);
      return { found: true };
    }),
  ]);

  const action = await execution(state([graph]), harness.context, handlers());

  assert.equal(
    action.type,
    'transition',
    action.type === 'fail' ? String(action.error) : undefined,
  );
  assert.equal(node.status, 'completed');
  assert.deepEqual(calls, [{ query: 'evidence' }]);
  assert.equal(provider.requests.length, 2);
  assert.equal(provider.requests[0]?.tools?.[0]?.name, 'lookup');
  assert.equal(provider.requests[0]?.schema, undefined);
  assert.equal(provider.requests[1]?.schema, undefined);
  assert.equal(provider.requests[0]?.tools?.length, 2);
  if (action.type !== 'transition') return;
  assert.deepEqual(node.outcome?.observations, [
    {
      goalId: 'current',
      toolName: 'lookup',
      callId: 'call-1',
      input: '{"query":"evidence"}',
      output: '{"found":true}',
    },
  ]);
});

test('automatically records every returned observation in tool-result order', async () => {
  const node = createNode('current', ['first', 'second']);
  const graph: Graph = { revision: 1, nodes: [node] };
  const provider = createProvider([
    toolCallsFinish([
      { id: 'call-first', name: 'first', query: 'alpha' },
      { id: 'call-second', name: 'second', query: 'beta' },
    ]),
    terminalFinish(completed()),
  ]);
  const harness = createHarness(graph, provider, [
    tool('first', async ({ query }) => ({ value: `${String(query)} result` })),
    tool('second', async ({ query }) => ({ value: `${String(query)} result` })),
  ]);

  const action = await execution(state([graph]), harness.context, handlers());

  assert.equal(action.type, 'transition');
  if (action.type !== 'transition') return;
  assert.equal(node.status, 'completed');
  assert.deepEqual(
    node.outcome?.observations.map(({ toolName, callId, input, output }) => ({
      toolName,
      callId,
      input,
      output,
    })),
    [
      {
        toolName: 'first',
        callId: 'call-first',
        input: '{"query":"alpha"}',
        output: '{"value":"alpha result"}',
      },
      {
        toolName: 'second',
        callId: 'call-second',
        input: '{"query":"beta"}',
        output: '{"value":"beta result"}',
      },
    ],
  );
});

test('blocks on turn exhaustion after retaining ordered observations without artifacts', async () => {
  const node = createNode('bounded', ['first', 'second']);
  const graph: Graph = { revision: 1, nodes: [node] };
  const provider = createProvider([
    toolCallsFinish([
      { id: 'call-first', name: 'first', query: 'alpha' },
      { id: 'call-second', name: 'second', query: 'beta' },
    ]),
  ]);
  const harness = createHarness(
    graph,
    provider,
    [
      tool('first', async () => ({ value: 'first result' })),
      tool('second', async () => ({ value: 'second result' })),
    ],
    [],
    [],
    1,
  );

  const action = await execution(state([graph]), harness.context, handlers());

  assert.equal(action.type, 'transition');
  if (action.type !== 'transition') return;
  assert.equal(provider.requests.length, 1);
  assert.equal(node.status, 'blocked');
  assert.equal(node.outcome, null);
  assert.deepEqual(node.artifacts, []);
  assert.deepEqual(
    node.termination?.type === 'turn_limit'
      ? node.termination.observations.map(({ toolName, callId, output }) => ({
          toolName,
          callId,
          output,
        }))
      : [],
    [
      {
        toolName: 'first',
        callId: 'call-first',
        output: '{"value":"first result"}',
      },
      {
        toolName: 'second',
        callId: 'call-second',
        output: '{"value":"second result"}',
      },
    ],
  );
  assert.deepEqual(harness.logs.at(-1), {
    bindings: { nodeId: 'bounded', status: 'blocked' },
    message: 'node execution did not complete',
  });
});

test('resolves selected skill references without treating rationales as instructions', async () => {
  const node = createNode('current');
  node.candidates = [
    {
      skillName: 'selected',
      score: 1,
      rank: 1,
      rationale: 'private rationale',
    },
    {
      skillName: 'rejected',
      score: 0.5,
      rank: 2,
      rationale: 'private rejected rationale',
    },
  ];
  node.bundle = {
    goalId: 'current',
    skills: ['selected'],
    selectionRationale: 'private global rationale',
  };
  const graph: Graph = { revision: 1, nodes: [node] };
  const provider = createProvider([terminalFinish(completed())]);
  const harness = createHarness(
    graph,
    provider,
    [],
    [skill('selected'), skill('rejected')],
    [skill('universal')],
  );

  const action = await execution(state([graph]), harness.context, handlers());

  assert.equal(action.type, 'transition');
  const system = provider.requests[0]?.messages[0]?.content ?? '';
  const user = provider.requests[0]?.messages
    .filter(({ role }) => role === 'user')
    .map(({ content }) => content ?? '')
    .join('\n');
  assert.equal(typeof system, 'string');
  assert.equal(typeof user, 'string');
  if (typeof system !== 'string' || typeof user !== 'string') return;
  assert.match(system, /universal body/u);
  assert.match(user, /selected body/u);
  assert.doesNotMatch(
    user,
    /rejected body|private rationale|private rejected rationale|private global rationale/u,
  );
});

test('preserves each semantic terminal status and resolves the wave', async () => {
  for (const status of ['blocked', 'failed'] as const) {
    const node = createNode(`node-${status}`);
    const graph: Graph = { revision: 1, nodes: [node] };
    const provider = createProvider([
      terminalFinish(nonCompleted(status, `${status} private reason`)),
    ]);
    const harness = createHarness(graph, provider);

    const action = await execution(state([graph]), harness.context, handlers());

    assert.equal(action.type, 'transition');
    if (action.type !== 'transition') continue;
    assert.equal(node.status, status);
    assert.equal(node.outcome?.status, status);
    assert.equal(node.outcome?.reason, `${status} private reason`);
    assert.deepEqual(node.artifacts, []);
  }
});

test('stores needs_revision with every node observation and does not promote a partial result', async () => {
  const node = createNode('current', ['first', 'middle', 'last']);
  const graph: Graph = { revision: 1, nodes: [node] };
  const provider = createProvider([
    toolCallsFinish([
      { id: 'call-first', name: 'first', query: 'first' },
      { id: 'call-middle', name: 'middle', query: 'invalidating' },
      { id: 'call-last', name: 'last', query: 'last' },
    ]),
    terminalFinish({
      ...nonCompleted('needs_revision', 'Revision required.'),
      result: { markdown: 'Unpromoted partial result.', artifacts: [] },
      revisionRequest: {
        goalId: 'current',
        invalidatedAssumption: 'The resource exists.',
        requestedEffect: 'Replace the resource-dependent goal.',
      },
    }),
  ]);
  const harness = createHarness(graph, provider, [
    tool('first', async () => ({ stage: 'context' })),
    tool('middle', async () => ({ exists: false })),
    tool('last', async () => ({ stage: 'confirmation' })),
  ]);

  const action = await execution(state([graph]), harness.context, handlers());

  assert.equal(action.type, 'transition');
  if (action.type !== 'transition') return;
  assert.equal(action.handler, 'schedule');
  assert.equal(node.status, 'needs_revision');
  assert.deepEqual(node.artifacts, []);
  assert.deepEqual(
    node.outcome?.observations.map(({ toolName, output }) => ({
      toolName,
      output,
    })),
    [
      { toolName: 'first', output: '{"stage":"context"}' },
      { toolName: 'middle', output: '{"exists":false}' },
      { toolName: 'last', output: '{"stage":"confirmation"}' },
    ],
  );
  assert.equal(node.outcome?.revisionRequest?.goalId, 'current');
  assert.equal(node.outcome?.observations.length, 3);
});

test('fails needs_revision when the node produced no observation', async () => {
  const node = createNode('current');
  const graph: Graph = { revision: 1, nodes: [node] };
  const provider = createProvider([
    terminalFinish({
      ...nonCompleted('needs_revision', 'Revision required.'),
      revisionRequest: {
        goalId: 'current',
        invalidatedAssumption: 'The plan assumption is invalid.',
        requestedEffect: 'Revise the plan.',
      },
    }),
  ]);
  const harness = createHarness(graph, provider);

  const action = await execution(state([graph]), harness.context, handlers());

  assert.equal(action.type, 'fail');
  if (action.type !== 'fail') return;
  assert.equal(node.status, 'running');
  assert.match(
    (action.error as Error).message,
    /requires at least one observation/u,
  );
});

test('propagates provider and tool failures with their exact identity', async () => {
  const providerError = new Error('schema rejected');
  const providerNode = createNode('provider');
  const providerGraph: Graph = { revision: 1, nodes: [providerNode] };
  const provider = createProvider([providerError]);
  const providerHarness = createHarness(providerGraph, provider);

  const providerAction = await execution(
    state([providerGraph]),
    providerHarness.context,
    handlers(),
  );

  assert.equal(providerAction.type, 'fail');
  if (providerAction.type === 'fail') {
    assert.strictEqual(providerAction.error, providerError);
  }
  assert.equal(providerNode.status, 'running');

  const toolNode = createNode('tool', ['lookup']);
  const toolGraph: Graph = { revision: 1, nodes: [toolNode] };
  const toolProvider = createProvider([toolFinish('call-failure', 'lookup')]);
  const toolError = new Error('tool payload detail');
  const toolHarness = createHarness(toolGraph, toolProvider, [
    tool('lookup', async () => {
      throw toolError;
    }),
  ]);

  const toolAction = await execution(
    state([toolGraph]),
    toolHarness.context,
    handlers(),
  );

  assert.equal(toolAction.type, 'fail');
  if (toolAction.type === 'fail') {
    assert.strictEqual((toolAction.error as Error).cause, toolError);
  }
  assert.equal(toolNode.status, 'running');
});

test('waits for the whole concurrent wave and resolves semantic outcomes', async () => {
  const completedNode = createNode('completed');
  const blockedNode = createNode('blocked');
  const graph: Graph = { revision: 1, nodes: [completedNode, blockedNode] };
  let completionIndex = 0;
  const provider = createProvider([], (request) => {
    const current = completionIndex++;
    const terminal =
      current === 0
        ? terminalFinish(completed())
        : terminalFinish(nonCompleted('blocked', 'No useful action.'));
    return terminal(request);
  });
  const harness = createHarness(graph, provider);

  const action = await execution(state([graph]), harness.context, handlers());

  assert.equal(action.type, 'transition');
  assert.equal(completedNode.status, 'completed');
  assert.equal(blockedNode.status, 'blocked');
});

type Step =
  | ProviderFinished<unknown>
  | Error
  | ((request: ProviderRequest<unknown>) => ProviderFinished<unknown>);

function createProvider(
  steps: Step[],
  complete?: (request: ProviderRequest<unknown>) => ProviderFinished<unknown>,
) {
  const requests: ProviderRequest<unknown>[] = [];
  let index = 0;

  const provider = {
    metadata: {
      id: 'fake',
      name: 'Fake',
      baseUrl: 'https://fake.invalid',
    },
    complete: async (request: ProviderRequest<unknown>) => {
      requests.push(request);
      const selected = complete?.(request) ?? steps[index++];
      const step =
        typeof selected === 'function' ? selected(request) : selected;
      if (step instanceof Error) throw step;
      if (step === undefined) throw new Error('Missing fake provider step.');
      return step;
    },
  } as unknown as LlmProvider;

  return { provider, requests };
}

function createHarness(
  graph: Graph,
  fake: ReturnType<typeof createProvider>,
  tools: readonly Tool[] = [],
  skills: readonly Skill[] = [],
  requiredSkills: readonly Skill[] = [],
  maxTurns = 8,
) {
  const logs: unknown[] = [];
  const context: WorkflowContext = {
    input: 'Complete the request.',
    options: {
      logger: {
        info: (bindings: unknown, message: string) =>
          logs.push({ bindings, message }),
      } as never,
      provider: fake.provider,
      models: {
        default: 'default-model',
        reranker: 'reranker-model',
      },
      routing: {
        maxHintCandidates: 5,
        maxRetrievedCandidates: 5,
        maxSkills: 5,
      },
      execution: { maxTurns },
      revision: { max: 3 },
      skills: {
        required: requiredSkills,
        menu: [...requiredSkills, ...skills],
        retriever: {} as never,
      },
      tools: {
        required: [],
        menu: tools,
        retriever: {} as never,
      },
    },
  };

  void graph;
  return { context, logs };
}

function skill(name: string): Skill {
  return {
    name,
    description: `${name} description`,
    body: `${name} body`,
    allowedTools: [],
    indexText: `${name} | ${name} description |  | ${name} body`,
  };
}

function state(graphs: Graph[]): WorkflowState {
  return { graphs };
}

function handlers() {
  return {
    transition: (handler: string, state: WorkflowState) => ({
      type: 'transition' as const,
      handler,
      state,
    }),
    finish: () => ({ type: 'finish' as const, value: undefined }),
    fail: (error: unknown) => ({ type: 'fail' as const, error }),
  } as never;
}

function createNode(id: string, toolNames: readonly string[] = []): Node {
  return {
    id,
    goal: `Goal ${id}`,
    doneWhen: [`${id} is complete.`],
    dependsOn: [],
    status: 'ready',
    deliver: true,
    index: 0,
    candidates: [],
    bundle: {
      goalId: id,
      skills: [],
      selectionRationale: 'No skills are needed.',
    },
    tools: toolNames.map((name) => ({ name, description: `${name} tool` })),
    artifacts: [],
    outcome: null,
    termination: null,
  };
}

function completed(overrides: Record<string, unknown> = {}) {
  return {
    status: 'completed',
    criteria: [
      { criterionIndex: 0, satisfied: true, evidence: 'Criterion met.' },
    ],
    result: { markdown: 'Completed.', artifacts: [] },
    revisionRequest: null,
    reason: null,
    ...overrides,
  };
}

function nonCompleted(
  status: 'needs_revision' | 'blocked' | 'failed',
  reason: string,
) {
  return {
    status,
    criteria: [
      { criterionIndex: 0, satisfied: false, evidence: 'Criterion unmet.' },
    ],
    result: null,
    revisionRequest: null,
    reason,
  };
}

function toolFinish(id: string, name: string): ProviderFinished<unknown> {
  return toolCallsFinish([{ id, name, query: 'evidence' }]);
}

function toolCallsFinish(
  calls: readonly {
    readonly id: string;
    readonly name: string;
    readonly query: string;
  }[],
): ProviderFinished<unknown> {
  return {
    text: '',
    finishReason: 'tool_calls',
    toolCalls: calls.map(({ id, name, query }) => ({
      id,
      name,
      arguments: JSON.stringify({ query }),
    })),
  };
}

function terminalFinish(
  value: unknown,
): (request: ProviderRequest<unknown>) => ProviderFinished<unknown> {
  return (request) => {
    const terminal = request.tools?.find(
      ({ description }) =>
        description ===
        'Submit the final structured output and end the agent run.',
    );

    assert.ok(terminal);
    return {
      text: '',
      finishReason: 'tool_calls',
      toolCalls: [
        {
          id: 'call-structured-output',
          name: terminal.name,
          arguments: JSON.stringify(value),
        },
      ],
    };
  };
}

function tool(
  name: string,
  execute: Tool['execute'] = async () => ({ found: true }),
): Tool {
  const input = z.object({ query: z.string() });
  const output = z.unknown();

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
      outputSchema: {},
      strict: true,
    },
    execute,
  };
}
