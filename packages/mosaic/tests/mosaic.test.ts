import assert from 'node:assert/strict';
import test from 'node:test';

import type { Skill } from 'bundle';
import type { Tool } from 'tool';

import mosaicDefault, {
  mosaic,
  type MosaicAgent,
  type MosaicOptions,
} from '../src/index.js';
import { selectWave } from '../src/lib/states/schedule/index.js';

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
};

type TestGraph = {
  readonly revision: string;
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

test('routes models and composes menus before preserving the missing-ready failure', async () => {
  const graph = createGraph([createNode('goal', 0)]);
  const harness = createHarness({
    graph,
    selected: ['selected-skill', 'required-skill', 'selected-skill'],
  });

  const { logs, value: error } = await captureConsole(() =>
    rejectionOf(harness.agent.prompt('Do it.')),
  );

  assert.ok(error instanceof Error);
  assert.equal(error.message, 'Impossible to continue, missing ready nodes!');
  assert.deepEqual(harness.completionModels, [
    'default-model',
    'default-model',
    'default-model',
    'default-model',
  ]);
  assert.deepEqual(harness.completionKinds, [
    'goals',
    'hints',
    'revision',
    'bundle',
  ]);
  assert.deepEqual(
    harness.searches.map(({ topK }) => topK),
    [3, 10],
  );
  assert.equal(harness.reranks.length, 1);
  assert.equal(harness.reranks[0]?.model, 'reranker-model');
  assert.equal(harness.reranks[0]?.topN, 10);
  assert.deepEqual(
    JSON.parse(logs[2] ?? '[]').map(({ name }: Skill) => name),
    ['required-skill', 'selected-skill'],
  );
  assert.deepEqual(
    JSON.parse(logs[3] ?? '[]').map(({ name }: Tool) => name),
    ['required-tool', 'required-enabled', 'selected-tool', 'shared'],
  );
  assert.equal(graph.nodes[0]?.status, 'pending');
  assert.equal(harness.embeddingCalls, 0);
});

test('selects at most five ready nodes in descending index order without mutating the source graph', () => {
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
    result.graph.nodes.map(({ status }) => status),
    ['pending', 'ready', 'ready', 'ready', 'ready', 'ready'],
  );
  assert.deepEqual(
    graph.nodes.map(({ status }) => status),
    ['pending', 'pending', 'pending', 'pending', 'pending', 'pending'],
  );
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

  const second = selectWave(first.graph);

  assert.equal(second.status, 'failed');
  if (second.status !== 'failed') return;
  assert.equal(
    second.error.message,
    'Impossible to continue, missing ready nodes!',
  );
});

test('finishes completed graphs without preparing nodes', async () => {
  const graph = createGraph([createNode('done', 0, [], 'completed')]);
  const harness = createHarness({ graph, matches: [] });

  const { logs, value } = await captureConsole(() =>
    harness.agent.prompt('Already done.'),
  );

  assert.equal(value, undefined);
  assert.deepEqual(harness.completionKinds, ['goals', 'revision']);
  assert.deepEqual(
    harness.searches.map(({ topK }) => topK),
    [3],
  );
  assert.deepEqual(harness.reranks, []);
  assert.deepEqual(logs, [JSON.stringify(graph)]);
});

test('rejects with the exact provider error object', async () => {
  const providerError = new Error('provider unavailable');
  const harness = createHarness({
    graph: createGraph([createNode('goal', 0)]),
    failure: { kind: 'rerank', error: providerError },
  });

  const { value: error } = await captureConsole(() =>
    rejectionOf(harness.agent.prompt('Do it.')),
  );

  assert.strictEqual(error, providerError);
});

test('supports repeated and concurrent prompts on the same agent', async () => {
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

  const { logs } = await captureConsole(async () => {
    await Promise.all([
      harness.agent.prompt('First request.'),
      harness.agent.prompt('Second request.'),
    ]);
    await harness.agent.prompt('Third request.');
  });

  assert.equal(logs.length, 3);
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
    [3, 3, 3],
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
    logger: { info: () => undefined } as never,
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
  revision: 'P1',
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
