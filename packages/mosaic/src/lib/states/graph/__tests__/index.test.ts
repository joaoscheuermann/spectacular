import assert from 'node:assert/strict';
import test from 'node:test';

import type { Skill } from 'bundle';

import { StrictGraphSchema } from '../../../schemas/graph.js';
import { SkillHintExtractionSchema } from '../../../schemas/hint.js';
import type { Graph, Node } from '../../../types/graph.js';
import type { MosaicOptions } from '../../../types/mosaic-options.js';
import { graph } from '../index.js';

const skill: Skill = {
  name: 'planning-skill',
  description: 'A planning skill.',
  body: 'Use a concrete intermediate result.',
  allowedTools: [],
};
const alternateSkill: Skill = { ...skill, name: 'alternate-skill' };

test('creates an initial graph without searching for skill hints', async () => {
  const plan = createGraph([createNode('initial')]);
  const completions: unknown[] = [];
  const searches: unknown[] = [];
  const action = await graph(
    { graphs: [] },
    { input: 'Build it.', options: options({ plan, completions, searches }) },
    handlers(),
  );

  assert.deepEqual(action, {
    type: 'transition',
    handler: 'graph',
    state: { graphs: [plan] },
  });
  assert.equal(completions.length, 1);
  assert.equal(searches.length, 0);
  assert.equal((completions[0] as { schema: unknown }).schema, StrictGraphSchema);
});

test('extracts non-empty hints and revises the active graph', async () => {
  const active = createGraph([createNode('current')]);
  const revised = createGraph([createNode('revised')]);
  const completions: Array<{ schema: unknown; messages: readonly { content?: string }[] }> = [];
  const searches: Array<{ query: string; topK: number }> = [];
  const action = await graph(
    { graphs: [active] },
    {
      input: 'Improve it.',
      options: options({
        plan: revised,
        completions,
        searches,
        matches: [skill, alternateSkill],
        hintResults: [[{ effect: 'gap', evidence: 'A result is missing.' }], []],
      }),
    },
    handlers(),
  );

  assert.equal(action.type, 'transition');
  if (action.type !== 'transition') return;
  assert.equal(action.handler, 'bundle');
  assert.deepEqual(action.state, { graphs: [active, revised] });
  assert.deepEqual(searches.map(({ topK }) => topK), [10]);
  assert.equal(completions.length, 3);
  assert.equal(completions[0]?.schema, SkillHintExtractionSchema);
  assert.equal(completions[1]?.schema, SkillHintExtractionSchema);
  assert.equal(completions[2]?.schema, StrictGraphSchema);
  assert.match(completions[0]?.messages[1]?.content ?? '', /planning-skill/);
});

test('reports provider failures through the workflow failure action', async () => {
  const failure = new Error('provider unavailable');
  const optionsValue = options({
    plan: createGraph([createNode('unused')]),
    completions: [],
    searches: [],
    failure,
  });
  const action = await graph(
    { graphs: [] },
    { input: 'Build it.', options: optionsValue },
    handlers(),
  );

  assert.deepEqual(action, { type: 'fail', error: failure });
});

function options(input: {
  readonly plan: Graph;
  readonly completions: unknown[];
  readonly searches: unknown[];
  readonly matches?: readonly Skill[];
  readonly hintResults?: readonly (readonly unknown[])[];
  readonly failure?: Error;
}): MosaicOptions {
  let hintIndex = 0;

  const provider = {
    complete: async (request: {
      readonly schema?: unknown;
      readonly messages: readonly { content?: string }[];
    }) => {
      input.completions.push(request);
      if (input.failure) throw input.failure;

      if (request.schema === SkillHintExtractionSchema) {
        return {
          structured: {
            hints: input.hintResults?.[hintIndex++] ?? [],
          },
        };
      }

      return { structured: input.plan };
    },
  };
  const embeddings = {
    search: async (query: string, topK: number) => {
      (input.searches as Array<{ query: string; topK: number }>).push({
        query,
        topK,
      });
      return (input.matches ?? [skill]).map((data) => ({ data, score: 1 }));
    },
  };

  return {
    logger: { debug: () => undefined } as never,
    provider: provider as never,
    models: { default: 'default-model', reranker: 'unused', embedder: 'unused' },
    skills: { required: [], menu: [skill], embeddings: embeddings as never },
    tools: { required: [], menu: [], embeddings: {} as never },
  };
}

function handlers() {
  return {
    transition: (handler: string, state: unknown) => ({
      type: 'transition' as const,
      handler,
      state,
    }),
    fail: (error: unknown) => ({ type: 'fail' as const, error }),
  } as never;
}

function createGraph(nodes: Node[]): Graph {
  return { nodes };
}

function createNode(id: string): Node {
  return {
    id,
    goal: `Goal ${id}`,
    doneWhen: [`${id} is complete.`],
    dependsOn: [],
    status: 'pending',
    deliver: true,
    index: 0,
    skills: [],
    tools: [],
    artifacts: [],
  };
}
