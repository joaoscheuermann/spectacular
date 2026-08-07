import assert from 'node:assert/strict';
import test from 'node:test';

import type { Skill } from 'bundle';
import { z } from 'zod';
import type { Tool } from 'tool';

import * as executionPrompt from '../src/lib/prompts/execution.js';
import type { Graph, Node } from '../src/lib/types/graph.js';

test('defines precedence tools criteria and all terminal statuses', () => {
  const prompt = executionPrompt.system([
    skill('universal', 'universal behavior'),
  ]);

  assert.match(prompt, /# Instruction precedence/u);
  assert.match(prompt, /# Tool use/u);
  assert.match(prompt, /zero-based criterionIndex/u);
  for (const status of ['completed', 'needs_revision', 'blocked', 'failed']) {
    assert.match(prompt, new RegExp(`- ${status}:`, 'u'));
  }
  assert.match(prompt, /structured-output mechanism supplied by/u);
  assert.match(prompt, /universal behavior/u);
  assert.doesNotMatch(prompt, /observationRefs|triggerObservationRef|callId/u);
});

test('projects only transitive ancestor artifacts and preserves skill order', () => {
  const root = createNode('root', 0, [], 'completed', 'root artifact');
  const direct = createNode(
    'direct',
    1,
    ['root'],
    'completed',
    'direct artifact',
  );
  const unrelated = createNode(
    'unrelated',
    2,
    [],
    'completed',
    'unrelated artifact',
  );
  const current = createNode('current', 3, ['direct'], 'ready');
  current.skills = [
    { skill: 'first', rationale: 'First is needed.' },
    { skill: 'second', rationale: 'Second is needed.' },
  ];
  current.tools = [{ name: 'lookup', description: 'Lookup evidence.' }];
  const graph: Graph = { nodes: [root, unrelated, direct, current] };

  const prompt = executionPrompt.user({
    request: 'Original request.',
    node: current,
    graph,
    skills: [skill('first', 'first body'), skill('second', 'second body')],
    tools: [tool('lookup')],
  });

  assert.match(prompt, /root artifact/u);
  assert.match(prompt, /direct artifact/u);
  assert.doesNotMatch(prompt, /unrelated artifact/u);
  assert.ok(prompt.indexOf('first body') < prompt.indexOf('second body'));
  assert.match(prompt, /Input schemas are supplied directly by the runtime/u);
  assert.doesNotMatch(prompt, /inputSchema/u);
});

test('uses collision-safe fences for arbitrary dynamic content', () => {
  const hostile = 'before\n``````\n~~~~~~\nafter';
  const current = createNode('current', 0, [], 'ready');
  current.goal = hostile;
  current.doneWhen = [hostile];
  current.skills = [{ skill: hostile, rationale: 'Needed.' }];
  const graph: Graph = { nodes: [current] };

  const prompt = executionPrompt.user({
    request: hostile,
    node: current,
    graph,
    skills: [skill(hostile, hostile)],
    tools: [tool(hostile)],
  });
  const occurrences = prompt.split(hostile).length - 1;

  assert.equal(occurrences, 8);
  assert.match(prompt, /`{7}text\nbefore/u);
  assert.doesNotMatch(prompt, /^\{\s*"/u);
});

function createNode(
  id: string,
  index: number,
  dependsOn: string[],
  status: Node['status'],
  artifact?: string,
): Node {
  return {
    id,
    goal: `Goal ${id}`,
    doneWhen: [`${id} is complete.`],
    dependsOn,
    status,
    deliver: true,
    index,
    skills: [],
    tools: [],
    artifacts:
      artifact === undefined ? [] : [{ mime: 'text/plain', data: artifact }],
    observations: [],
    revisionRequest: null,
  };
}

function skill(name: string, body: string): Skill {
  return {
    name,
    description: `${name} description`,
    body,
    allowedTools: [],
  };
}

function tool(name: string): Tool {
  return {
    name,
    description: `${name} description`,
    schema: z.object({ query: z.string() }),
    definition: {
      name,
      description: `${name} description`,
      inputSchema: {
        type: 'object',
        properties: { query: { type: 'string' } },
        required: ['query'],
        additionalProperties: false,
      },
      strict: true,
    },
    execute: async () => 'result',
  };
}
