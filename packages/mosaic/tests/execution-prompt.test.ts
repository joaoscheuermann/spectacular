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
  current.candidates = [
    candidate('first', 1, 'First is needed.'),
    candidate('second', 2, 'Second is needed.'),
  ];
  current.bundle = {
    goalId: 'current',
    skills: ['first', 'second'],
    selectionRationale: 'Both skills are needed.',
  };
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
  current.candidates = [candidate(hostile, 1, 'Needed.')];
  current.bundle = {
    goalId: 'current',
    skills: [hostile],
    selectionRationale: 'The skill is needed.',
  };
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
    candidates: [],
    bundle: null,
    tools: [],
    artifacts:
      artifact === undefined ? [] : [{ mime: 'text/plain', data: artifact }],
    outcome: null,
    termination: null,
  };
}

function skill(name: string, body: string): Skill {
  return {
    name,
    description: `${name} description`,
    body,
    allowedTools: [],
    indexText: `${name} | ${name} description |  | ${body}`,
  };
}

function candidate(skillName: string, rank: number, rationale: string) {
  return { skillName, score: 1, rank, rationale };
}

function tool(name: string): Tool {
  return {
    name,
    description: `${name} description`,
    input: z.object({ query: z.string() }),
    output: z.string(),
    definition: {
      name,
      description: `${name} description`,
      inputSchema: {
        type: 'object',
        properties: { query: { type: 'string' } },
        required: ['query'],
        additionalProperties: false,
      },
      outputSchema: { type: 'string' },
      strict: true,
    },
    execute: async () => 'result',
  };
}
