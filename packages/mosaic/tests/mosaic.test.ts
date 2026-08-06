import assert from 'node:assert/strict';
import test from 'node:test';

import type { Skill } from 'bundle';
import type { Tool, ToolMetadata } from 'tool';

import mosaicDefault, {
  mosaic,
  type MosaicAgent,
  type MosaicOptions,
} from '../src/index.js';
import { schedule, selectWave } from '../src/lib/states/schedule/index.js';

type Status =
  | 'pending'
  | 'ready'
  | 'running'
  | 'completed'
  | 'needs_revision'
  | 'blocked'
  | 'failed';

type TestNode = {
  readonly id: string;
  readonly goal: string;
  readonly doneWhen: readonly string[];
  readonly dependsOn: readonly string[];
  readonly status: Status;
  readonly deliver: boolean;
  readonly index: number;
  readonly skills: readonly Skill[];
  readonly tools: readonly ToolMetadata[];
  readonly artifacts: readonly unknown[];
};

type TestGraph = {
  readonly nodes: readonly TestNode[];
};

type SearchCall = {
  readonly query: string;
  readonly topK: number;
};

type RerankCall = {
  readonly model: string;
  readonly query: string;
  readonly documents: readonly string[];
  readonly topN?: number;
};

type CompletionKind = 'goals' | 'hints' | 'revision' | 'bundle';

type HarnessOptions = {
  readonly graph: TestGraph | ((message: string) => TestGraph);
  readonly matches?: readonly Skill[];
  readonly failure?: {
    readonly kind: CompletionKind | 'rerank';
    readonly error: unknown;
  };
  readonly selected?: readonly string[];
};

const requiredSkill = skill('required-skill', ['required-enabled', 'shared']);
const selectedSkill = skill('selected-skill', ['selected-tool', 'shared']);
const requiredTool = tool('required-tool');
const requiredEnabled = tool('required-enabled');
const selectedTool = tool('selected-tool');
const sharedTool = tool('shared');

test('exports the same factory as named and default with an async prompt', () => {
  assert.strictEqual(mosaicDefault, mosaic);
  const agent: MosaicAgent = { prompt: async () => undefined };
  assert.ok(agent.prompt('request') instanceof Promise);
});

test('revises the graph before the bundle state reports its missing active graph', async () => {
  const graph = createGraph([createNode('goal', 0)]);
  const harness = createHarness({
    graph,
    selected: ['selected-skill', 'required-skill', 'selected-skill'],
  });

  const { logs, value: error } = await captureConsole(() =>
    rejectionOf(harness.agent.prompt('Do it.')),
  );

  assert.ok(error instanceof Error);
  assert.equal(error.message, 'Impossible to continue, missing active graph!');
  assert.deepEqual(harness.completionModels, [
    'default-model',
    'default-model',
    'default-model',
  ]);
  assert.deepEqual(harness.completionKinds, [
    'goals',
    'hints',
    'revision',
  ]);
  assert.deepEqual(
    harness.searches.map(({ topK }) => topK),
    [10],
  );
  assert.deepEqual(harness.reranks, []);
  assert.deepEqual(logs, []);
  assert.equal(graph.nodes[0]?.status, 'pending');
  assert.equal(harness.embeddingCalls, 0);
});

test('marks at most five nodes ready in descending index order', () => {
  const graph = createGraph(
    Array.from({ length: 6 }, (_, index) => createNode(`goal-${index}`, index)),
  );

  const result = selectWave(graph as never);

  assert.equal(result.status, 'ready');
  if (result.status !== 'ready') return;
  assert.deepEqual(
    result.nodes.map(({ id }) => id),
    ['goal-5', 'goal-4', 'goal-3', 'goal-2', 'goal-1'],
  );
  assert.deepEqual(
    graph.nodes.map(({ status }) => status),
    ['pending', 'ready', 'ready', 'ready', 'ready', 'ready'],
  );
});

test('schedules the last graph in the workflow state', async () => {
  const older = createGraph([createNode('older', 0)]);
  const active = createGraph([createNode('active', 0)]);
  const graphs = [older, active];

  const action = await schedule(
    { graphs } as never,
    {} as never,
    {
      transition: (handler: string, state: object) => ({
        type: 'transition',
        handler,
        state,
      }),
      finish: () => ({ type: 'finish', value: undefined }),
      fail: (error: unknown) => ({ type: 'fail', error }),
    } as never,
  );

  assert.equal(action.type, 'transition');
  assert.equal(older.nodes[0]?.status, 'pending');
  assert.equal(active.nodes[0]?.status, 'ready');
});

test('keeps dependent nodes blocked when the prerequisite wave is only ready', () => {
  const graph = createGraph([
    createNode('prerequisite', 0),
    createNode('dependent', 1, ['prerequisite']),
  ]);

  const first = selectWave(graph as never);

  assert.equal(first.status, 'ready');
  if (first.status !== 'ready') return;
  assert.deepEqual(
    first.nodes.map(({ id }) => id),
    ['prerequisite'],
  );

  const second = selectWave(graph as never);

  assert.equal(second.status, 'failed');
  if (second.status !== 'failed') return;
  assert.equal(
    second.error.message,
    'Impossible to continue, missing ready nodes!',
  );
});

test('fails at the bundle state before scheduling a completed graph', async () => {
  const graph = createGraph([createNode('done', 0, [], 'completed')]);
  const harness = createHarness({ graph, matches: [] });

  const { logs, value: error } = await captureConsole(() =>
    rejectionOf(harness.agent.prompt('Already done.')),
  );

  assert.ok(error instanceof Error);
  assert.equal(error.message, 'Impossible to continue, missing active graph!');
  assert.deepEqual(harness.completionKinds, ['goals', 'revision']);
  assert.deepEqual(
    harness.searches.map(({ topK }) => topK),
    [10],
  );
  assert.deepEqual(harness.reranks, []);
  assert.deepEqual(logs, []);
});

test('rejects with the exact hint-provider error object', async () => {
  const providerError = new Error('provider unavailable');
  const harness = createHarness({
    graph: createGraph([createNode('goal', 0)]),
    failure: { kind: 'hints', error: providerError },
  });

  const { value: error } = await captureConsole(() =>
    rejectionOf(harness.agent.prompt('Do it.')),
  );

  assert.strictEqual(error, providerError);
});

test('keeps repeated and concurrent prompts isolated through the bundle failure', async () => {
  const harness = createHarness({
    graph: (message) =>
      createGraph([
        createNode(
          message.includes('First') ? 'first' : 'other',
          0,
          [],
          'completed',
        ),
      ]),
    matches: [],
  });

  const { logs, value: errors } = await captureConsole(async () => {
    const concurrent = await Promise.all([
      rejectionOf(harness.agent.prompt('First request.')),
      rejectionOf(harness.agent.prompt('Second request.')),
    ]);
    const third = await rejectionOf(harness.agent.prompt('Third request.'));

    return [...concurrent, third];
  });

  assert.ok(
    errors.every(
      (error) =>
        error instanceof Error &&
        error.message === 'Impossible to continue, missing active graph!',
    ),
  );
  assert.deepEqual(logs, []);
  assert.deepEqual(harness.completionKinds, [
    'goals',
    'goals',
    'revision',
    'revision',
    'goals',
    'revision',
  ]);
  assert.deepEqual(
    harness.searches.map(({ topK }) => topK),
    [10, 10, 10],
  );
});

function createHarness(options: HarnessOptions) {
  const searches: SearchCall[] = [];
  const reranks: RerankCall[] = [];
  const completionKinds: CompletionKind[] = [];
  const completionModels: string[] = [];
  let embeddingCalls = 0;

  const provider = {
    complete: async (request: {
      readonly model: string;
      readonly messages: readonly { readonly content?: string }[];
    }) => {
      const kind = completionKind(request.messages[0]?.content ?? '');
      completionKinds.push(kind);
      completionModels.push(request.model);

      if (options.failure?.kind === kind) {
        throw options.failure.error;
      }

      if (kind === 'hints') {
        return { structured: { hints: [] } };
      }

      if (kind === 'bundle') {
        return {
          structured: {
            goal: 'goal',
            skills: options.selected ?? ['selected-skill'],
            decisions: [],
            rationale: 'Enough.',
          },
        };
      }

      const message = request.messages.map(({ content }) => content).join('\n');
      return {
        structured:
          typeof options.graph === 'function'
            ? options.graph(message)
            : options.graph,
      };
    },
    rerank: async (request: RerankCall) => {
      reranks.push(request);

      if (options.failure?.kind === 'rerank') {
        throw options.failure.error;
      }

      return request.documents.map((_, index) => ({
        index,
        relevanceScore: 1,
      }));
    },
    embedding: async () => {
      embeddingCalls += 1;
      throw new Error('provider.embedding must not be accessed');
    },
  };
  const embeddings = {
    add: async () => undefined,
    search: async (query: string, topK: number) => {
      searches.push({ query, topK });
      return (options.matches ?? [selectedSkill]).map((data) => ({
        data,
        score: 1,
      }));
    },
  };
  const inaccessible = (name: string) =>
    new Proxy(
      {},
      {
        get() {
          throw new Error(`${name} must not be accessed`);
        },
      },
    );
  const mosaicOptions: MosaicOptions = {
    logger: { debug: () => undefined } as never,
    provider: provider as never,
    models: {
      default: 'default-model',
      reranker: 'reranker-model',
      embedder: 'unused-embedder',
    },
    skills: {
      required: [requiredSkill],
      menu: [requiredSkill, selectedSkill],
      embeddings,
    },
    tools: {
      required: [requiredTool],
      menu: [requiredTool, requiredEnabled, selectedTool, sharedTool],
      embeddings: inaccessible('tools.embeddings') as never,
    },
  };

  return {
    agent: mosaic(mosaicOptions),
    searches,
    reranks,
    completionKinds,
    completionModels,
    get embeddingCalls() {
      return embeddingCalls;
    },
  };
}

const completionKind = (system: string): CompletionKind => {
  if (system.includes('bundle selector')) return 'bundle';
  if (system.includes('candidate skill reveals')) return 'hints';
  if (system.includes('revise an initial')) return 'revision';
  return 'goals';
};

const createGraph = (nodes: readonly TestNode[]): TestGraph => ({
  nodes,
});

const createNode = (
  id: string,
  index: number,
  dependsOn: readonly string[] = [],
  status: Status = 'pending',
): TestNode => ({
  id,
  goal: `Goal ${id}`,
  doneWhen: [`${id} is done.`],
  dependsOn,
  status,
  deliver: true,
  index,
  skills: [],
  tools: [],
  artifacts: [],
});

function skill(name: string, allowedTools: readonly string[] = []): Skill {
  return {
    name,
    description: `${name} description`,
    body: `${name} body`,
    allowedTools,
  };
}

function tool(name: string): Tool {
  return {
    name,
    schema: {} as Tool['schema'],
    definition: { name, inputSchema: {} },
    execute: async () => undefined,
  };
}

const rejectionOf = async (promise: Promise<unknown>): Promise<unknown> => {
  try {
    await promise;
  } catch (error) {
    return error;
  }

  assert.fail('Expected the promise to reject.');
};

const captureConsole = async <Value>(
  run: () => Promise<Value>,
): Promise<{ readonly logs: string[]; readonly value: Value }> => {
  const logs: string[] = [];
  const original = console.log;
  console.log = (value?: unknown) => logs.push(String(value));

  try {
    return { logs, value: await run() };
  } finally {
    console.log = original;
  }
};
