import assert from 'node:assert/strict';
import test from 'node:test';

import type { LlmProvider, ProviderFinished, ProviderRequest } from 'llms';
import { z } from 'zod';
import type { Tool } from 'tool';

import { execution } from '../src/lib/states/execution/index.js';
import { revisionNodes } from '../src/lib/states/revision/localized.js';
import type { Graph, Node } from '../src/lib/types/graph.js';
import type {
  WorkflowContext,
  WorkflowState,
} from '../src/lib/types/workflow.js';
import { mosaicProviders } from './structured.js';

test('derives concurrent revision work in wave order rather than completion order', async () => {
  const first = node('first', 0, 'tool-first');
  const second = node('second', 1, 'tool-second');
  const graph: Graph = { revision: 1, nodes: [first, second] };
  const turns = new Map<string, number>();
  const provider = {
    metadata: { id: 'fake', name: 'Fake', baseUrl: 'https://fake.invalid' },
    complete: async (request: ProviderRequest<unknown>) => {
      const name = request.tools?.find(({ name }) =>
        name.startsWith('tool-'),
      )?.name;
      assert.ok(name);
      const goalId = name.replace('tool-', '');
      const turn = turns.get(goalId) ?? 0;
      turns.set(goalId, turn + 1);

      if (turn === 0) return toolCall(`call-${goalId}`, name);
      if (goalId === 'second') {
        await new Promise((resolve) => setTimeout(resolve, 10));
      }
      return terminal(request, revisionOutcome(goalId));
    },
  } as unknown as LlmProvider;

  const action = await execution(
    state(graph),
    context(provider, [tool('tool-first'), tool('tool-second')]),
    handlers(),
  );

  assert.equal(action.type, 'transition');
  if (action.type !== 'transition') return;
  const revisions = revisionNodes(graph);
  assert.deepEqual(
    revisions.map(({ outcome }) => outcome?.revisionRequest?.goalId),
    ['second', 'first'],
  );
  assert.deepEqual(
    revisions.flatMap(({ outcome }) =>
      (outcome?.observations ?? []).map(({ goalId }) => goalId),
    ),
    ['second', 'first'],
  );
});

const context = (provider: LlmProvider, tools: Tool[]): WorkflowContext => ({
  input: 'Complete the request.',
  options: {
    logger: { info: () => undefined } as never,
    providers: mosaicProviders(provider),
    models: {
      planning: { model: 'default', effort: 'low' },
      revision: { model: 'default', effort: 'low' },
      execution: { model: 'default', effort: 'low' },
      reranker: 'reranker',
      embedder: 'embedder',
    },
    routing: {
      maxHintCandidates: 2,
      maxRetrievedCandidates: 2,
      maxSkills: 0,
    },
    execution: { maxTurns: 8 },
    revision: { max: 3 },
    skills: { required: [], menu: [], retriever: {} as never },
    tools: { required: [], menu: tools, retriever: {} as never },
  },
});

const state = (graph: Graph): WorkflowState => ({
  graphs: [graph],
});

const node = (id: string, index: number, toolName: string): Node => ({
  id,
  goal: `Goal ${id}`,
  doneWhen: [`${id} is complete.`],
  dependsOn: [],
  status: 'ready',
  deliver: true,
  index,
  candidates: [],
  bundle: {
    goalId: id,
    skills: [],
    selectionRationale: 'No skills are needed.',
  },
  tools: [{ name: toolName, description: `${toolName} tool` }],
  artifacts: [],
  outcome: null,
  termination: null,
});

const tool = (name: string): Tool => {
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
    execute: async () => ({ found: true }),
  };
};

const toolCall = (id: string, name: string): ProviderFinished<unknown> => ({
  text: '',
  finishReason: 'tool_calls',
  toolCalls: [{ id, name, arguments: '{"query":"evidence"}' }],
});

const revisionOutcome = (goalId: string) => ({
  status: 'needs_revision',
  criteria: [
    { criterionIndex: 0, satisfied: false, evidence: 'Criterion unmet.' },
  ],
  result: null,
  revisionRequest: {
    goalId,
    invalidatedAssumption: 'The structure is valid.',
    requestedEffect: 'Revise the structure.',
  },
  reason: 'Revision required.',
});

const terminal = (
  request: ProviderRequest<unknown>,
  outcome: unknown,
): ProviderFinished<unknown> => {
  const definition = request.tools?.find(
    ({ description }) =>
      description ===
      'Submit the final structured output and end the agent run.',
  );
  assert.ok(definition);
  return {
    text: '',
    finishReason: 'tool_calls',
    toolCalls: [
      {
        id: 'call-structured-output',
        name: definition.name,
        arguments: JSON.stringify(outcome),
      },
    ],
  };
};

const handlers = () =>
  ({
    transition: (handler: string, value: WorkflowState) => ({
      type: 'transition' as const,
      handler,
      state: value,
    }),
    finish: () => ({ type: 'finish' as const, value: undefined }),
    fail: (error: unknown) => ({ type: 'fail' as const, error }),
  }) as never;
