import assert from 'node:assert/strict';
import test from 'node:test';

import type { Skill } from 'bundle';
import mosaicDefault, { mosaic, type MosaicAgent } from '../src/index.js';
import type { Tool } from 'tool';

const skill = (name: string, allowedTools: readonly string[] = []): Skill => ({
  name,
  description: `${name} description`,
  body: `${name} body`,
  allowedTools,
});

const tool = (name: string): Tool => ({
  name,
  schema: {} as Tool['schema'],
  definition: { name, inputSchema: {} },
  execute: async () => undefined,
});

test('exports the same factory as named and default with an async prompt', () => {
  assert.strictEqual(mosaicDefault, mosaic);
  const agent: MosaicAgent = { prompt: async () => undefined };
  assert.ok(agent.prompt('request') instanceof Promise);
});

test('routes models and composes node menus before preserving the incomplete failure', async () => {
  const requiredSkill = skill('required-skill', ['required-enabled', 'shared']);
  const selectedSkill = skill('selected-skill', ['selected-tool', 'shared']);
  const requiredTool = tool('required-tool');
  const requiredEnabled = tool('required-enabled');
  const selectedTool = tool('selected-tool');
  const sharedTool = tool('shared');
  const completionModels: string[] = [];
  const rerankModels: string[] = [];
  const searches: string[] = [];
  let completion = 0;
  const graph = {
    revision: 'P1',
    nodes: [
      {
        id: 'goal',
        goal: 'Produce result.',
        doneWhen: ['Result exists.'],
        dependsOn: [],
        status: 'pending' as const,
        deliver: true,
        index: 0,
      },
    ],
  };
  const provider = {
    complete: async (request: { model: string }) => {
      completionModels.push(request.model);
      completion += 1;
      if (completion === 1 || completion === 3) return { structured: graph };
      if (completion === 2) return { structured: { hints: [] } };
      return {
        structured: {
          goal: 'goal',
          skills: ['selected-skill', 'required-skill', 'selected-skill'],
          decisions: [],
          rationale: 'Enough.',
        },
      };
    },
    rerank: async (request: { model: string }) => {
      rerankModels.push(request.model);
      return [{ index: 0, relevanceScore: 1 }];
    },
  };
  const embeddings = {
    add: async () => undefined,
    search: async (query: string) => {
      searches.push(query);
      return [{ data: selectedSkill, score: 1 }];
    },
  };
  const toolEmbeddings = new Proxy(
    {},
    {
      get() {
        throw new Error('tools.embeddings must not be accessed');
      },
    },
  );
  const logs: string[] = [];
  const originalLog = console.log;
  console.log = (value?: unknown) => logs.push(String(value));

  try {
    const agent = mosaic({
      logger: { info: () => undefined } as never,
      provider: provider as never,
      session: new Proxy(
        {},
        {
          get() {
            throw new Error('session must not be accessed');
          },
        },
      ),
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
        embeddings: toolEmbeddings as never,
      },
    });

    await assert.rejects(
      agent.prompt('Do it.'),
      /Impossible to continue, missing ready nodes!/,
    );
  } finally {
    console.log = originalLog;
  }

  assert.deepEqual(completionModels, [
    'default-model',
    'default-model',
    'default-model',
    'default-model',
  ]);
  assert.deepEqual(rerankModels, ['reranker-model']);
  assert.equal(searches.length, 2);
  assert.deepEqual(
    JSON.parse(logs[2] ?? '[]').map(({ name }: Tool) => name),
    ['required-skill', 'selected-skill'],
  );
  assert.deepEqual(
    JSON.parse(logs[3] ?? '[]').map(({ name }: Tool) => name),
    ['required-tool', 'required-enabled', 'selected-tool', 'shared'],
  );
});
