import assert from 'node:assert/strict';
import test from 'node:test';

import type { LlmProvider, ProviderFinished, ProviderRequest } from 'llms';
import { z } from 'zod';
import type { Tool } from 'tool';

import { execution } from '../src/lib/states/execution/index.js';
import type { Graph, Node } from '../src/lib/types/graph.js';
import type {
  WorkflowContext,
  WorkflowState,
} from '../src/lib/types/workflow.js';

test('completes a node stores result artifacts and schedules the next wave', async () => {
  const node = createNode('current');
  const graph: Graph = { nodes: [node] };
  const provider = createProvider([
    finish(
      completed({
        result: {
          markdown: ' \n## Final result\n',
          artifacts: [{ mime: 'text/plain', data: 'extra' }],
        },
      }),
    ),
  ]);
  const harness = createHarness(graph, provider);

  const action = await execution(
    { graphs: [graph] },
    harness.context,
    handlers(),
  );

  assert.deepEqual(action, {
    type: 'transition',
    handler: 'schedule',
    state: { graphs: [graph] },
  });
  assert.equal(node.status, 'completed');
  assert.deepEqual(node.artifacts, [
    { mime: 'text/markdown', data: ' \n## Final result\n' },
    { mime: 'text/plain', data: 'extra' },
  ]);
  assert.equal(provider.requests.length, 1);
  assert.ok(provider.requests[0]?.schema);
  assert.equal(provider.requests[0]?.tools, undefined);
  assert.equal(provider.requests[0]?.messages[0]?.role, 'system');
  assert.equal(provider.requests[0]?.messages[1]?.role, 'user');
});

test('accepts only tool call IDs observed in the isolated message storage', async () => {
  const node = createNode('current', ['lookup']);
  const graph: Graph = { nodes: [node] };
  const provider = createProvider([
    toolFinish('call-1', 'lookup'),
    terminalFinish(completed({ observationRefs: ['call-1'] })),
  ]);
  const calls: unknown[] = [];
  const harness = createHarness(graph, provider, [
    tool('lookup', async (payload) => {
      calls.push(payload);
      return { found: true };
    }),
  ]);

  const action = await execution(
    { graphs: [graph] },
    harness.context,
    handlers(),
  );

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
});

test('rejects invented observation references and marks the node failed', async () => {
  const node = createNode('current');
  const graph: Graph = { nodes: [node] };
  const provider = createProvider([
    finish(completed({ observationRefs: ['invented-call'] })),
  ]);
  const harness = createHarness(graph, provider);

  const action = await execution(
    { graphs: [graph] },
    harness.context,
    handlers(),
  );

  assert.equal(action.type, 'fail');
  if (action.type !== 'fail') return;
  assert.equal(node.status, 'failed');
  assert.equal(
    (action.error as Error).message,
    'Node current returned an unknown observation reference.',
  );
});

test('preserves each non-completed status and fails with only node ID and status', async () => {
  for (const status of ['blocked', 'failed'] as const) {
    const node = createNode(`node-${status}`);
    const graph: Graph = { nodes: [node] };
    const provider = createProvider([
      finish(nonCompleted(status, `${status} private reason`)),
    ]);
    const harness = createHarness(graph, provider);

    const action = await execution(
      { graphs: [graph] },
      harness.context,
      handlers(),
    );

    assert.equal(action.type, 'fail');
    if (action.type !== 'fail') continue;
    assert.equal(node.status, status);
    assert.equal(
      (action.error as Error).message,
      `Node node-${status} ended with status ${status}.`,
    );
    assert.doesNotMatch((action.error as Error).message, /private reason/u);
    assert.deepEqual(node.artifacts, []);
  }
});

test('preserves needs_revision after validating its trigger observation', async () => {
  const node = createNode('current', ['lookup']);
  const graph: Graph = { nodes: [node] };
  const provider = createProvider([
    toolFinish('call-revision', 'lookup'),
    terminalFinish({
      ...nonCompleted('needs_revision', 'Revision required.'),
      result: { markdown: 'Unpromoted partial result.', artifacts: [] },
      observationRefs: ['call-revision'],
      revisionRequest: {
        goalId: 'current',
        triggerObservationRef: 'call-revision',
        invalidatedAssumption: 'The resource exists.',
        requestedEffect: 'Replace the resource-dependent goal.',
      },
    }),
  ]);
  const harness = createHarness(graph, provider, [tool('lookup')]);

  const action = await execution(
    { graphs: [graph] },
    harness.context,
    handlers(),
  );

  assert.equal(action.type, 'fail');
  if (action.type !== 'fail') return;
  assert.equal(
    node.status,
    'needs_revision',
    action.type === 'fail' ? String(action.error) : undefined,
  );
  assert.equal(
    (action.error as Error).message,
    'Node current ended with status needs_revision.',
  );
  assert.deepEqual(node.artifacts, []);
});

test('marks provider schema and tool failures as failed and propagates them', async () => {
  const providerError = new Error('schema rejected');
  const providerNode = createNode('provider');
  const providerGraph: Graph = { nodes: [providerNode] };
  const provider = createProvider([providerError]);
  const providerHarness = createHarness(providerGraph, provider);

  const providerAction = await execution(
    { graphs: [providerGraph] },
    providerHarness.context,
    handlers(),
  );

  assert.equal(providerAction.type, 'fail');
  if (providerAction.type === 'fail') {
    assert.strictEqual(providerAction.error, providerError);
  }
  assert.equal(providerNode.status, 'failed');

  const toolNode = createNode('tool', ['lookup']);
  const toolGraph: Graph = { nodes: [toolNode] };
  const toolProvider = createProvider([toolFinish('call-failure', 'lookup')]);
  const toolHarness = createHarness(toolGraph, toolProvider, [
    tool('lookup', async () => {
      throw new Error('tool payload detail');
    }),
  ]);

  const toolAction = await execution(
    { graphs: [toolGraph] },
    toolHarness.context,
    handlers(),
  );

  assert.equal(toolAction.type, 'fail');
  assert.equal(toolNode.status, 'failed');
});

test('waits for the whole concurrent wave before reporting a node outcome failure', async () => {
  const completedNode = createNode('completed');
  const blockedNode = createNode('blocked');
  const graph: Graph = { nodes: [completedNode, blockedNode] };
  let completionIndex = 0;
  const provider = createProvider([], () => {
    const current = completionIndex++;
    return current === 0
      ? finish(completed())
      : finish(nonCompleted('blocked', 'No useful action.'));
  });
  const harness = createHarness(graph, provider);

  const action = await execution(
    { graphs: [graph] },
    harness.context,
    handlers(),
  );

  assert.equal(action.type, 'fail');
  assert.equal(completedNode.status, 'completed');
  assert.equal(blockedNode.status, 'blocked');
});

type Step =
  | ProviderFinished<unknown>
  | Error
  | ((request: ProviderRequest<unknown>) => ProviderFinished<unknown>);

function createProvider(
  steps: Step[],
  complete?: () => ProviderFinished<unknown>,
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
      const selected = complete?.() ?? steps[index++];
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
        embedder: 'embedder-model',
      },
      skills: {
        required: [],
        menu: [],
        embeddings: {} as never,
      },
      tools: {
        required: [],
        menu: tools,
        embeddings: {} as never,
      },
    },
  };

  void graph;
  return { context, logs };
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
    skills: [],
    tools: toolNames.map((name) => ({ name, description: `${name} tool` })),
    artifacts: [],
  };
}

function completed(overrides: Record<string, unknown> = {}) {
  return {
    status: 'completed',
    criteria: [
      { criterionIndex: 0, satisfied: true, evidence: 'Criterion met.' },
    ],
    result: { markdown: 'Completed.', artifacts: [] },
    observationRefs: [],
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
    observationRefs: [],
    revisionRequest: null,
    reason,
  };
}

function finish(structured: unknown): ProviderFinished<unknown> {
  return {
    text: JSON.stringify(structured),
    finishReason: 'stop',
    toolCalls: [],
    structured,
  };
}

function toolFinish(id: string, name: string): ProviderFinished<unknown> {
  return {
    text: '',
    finishReason: 'tool_calls',
    toolCalls: [{ id, name, arguments: JSON.stringify({ query: 'evidence' }) }],
  };
}

function terminalFinish(value: unknown): Step {
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
  const schema = z.object({ query: z.string() });

  return {
    name,
    description: `${name} tool`,
    schema,
    definition: {
      name,
      description: `${name} tool`,
      inputSchema: {
        type: 'object',
        properties: { query: { type: 'string' } },
        required: ['query'],
        additionalProperties: false,
      },
      strict: true,
    },
    execute,
  };
}
