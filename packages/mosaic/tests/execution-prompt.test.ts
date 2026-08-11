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
  assert.match(prompt, /limit applies per response, not per node/u);
  assert.match(prompt, /failed command is an observation/u);
  assert.match(prompt, /another reasonable command or tool action/u);
  assert.match(prompt, /One missing executable/u);
  assert.match(prompt, /zero-based criterionIndex/u);
  assert.match(prompt, /smallest set of observationIndices/u);
  assert.match(prompt, /current node's observation ledger starts at index 0/u);
  assert.match(prompt, /first returned tool result.*index 0/u);
  assert.match(prompt, /Never use an ancestor's observation index/u);
  assert.match(
    prompt,
    /less than the number[\s\S]*of tool results returned during the current node/u,
  );
  assert.match(prompt, /exit_code.*stderr.*timed_out.*truncated/u);
  assert.match(
    prompt,
    /later successful command does not automatically resolve/u,
  );
  assert.match(prompt, /set -e.*&&/u);
  assert.match(prompt, /concrete evidence shows completion is impossible/u);
  assert.match(prompt, /reasonable[\s\S]*alternatives/u);
  assert.match(prompt, /explicit confirmation does not prove impossibility/u);
  for (const status of ['completed', 'needs_revision', 'blocked', 'failed']) {
    assert.match(prompt, new RegExp(`- ${status}:`, 'u'));
  }
  assert.match(prompt, /structured-output mechanism supplied by/u);
  assert.match(prompt, /universal behavior/u);
  assert.doesNotMatch(prompt, /observationRefs|triggerObservationRef|callId/u);
});

test('projects only cited transitive ancestor evidence and preserves skill order', () => {
  const root = completedNode('root', 0, [], 'root artifact', [0]);
  const direct = createNode(
    'direct',
    1,
    ['root'],
    'completed',
    'direct artifact',
  );
  direct.outcome = completedOutcome(
    'direct',
    [0, 1],
    [
      observation('direct', 'direct-call-1', 'cited direct output'),
      observation('direct', 'direct-call-2', 'also cited direct output'),
      observation('direct', 'direct-call-3', 'uncited direct output'),
    ],
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
  const graph: Graph = {
    revision: 1,
    nodes: [root, unrelated, direct, current],
  };

  const prompt = executionPrompt.user({
    request: 'Original request.',
    node: current,
    graph,
    skills: [skill('first', 'first body'), skill('second', 'second body')],
    tools: [tool('lookup')],
  });

  assert.match(prompt, /root artifact/u);
  assert.match(prompt, /direct artifact/u);
  assert.match(prompt, /cited root output/u);
  assert.match(prompt, /cited direct output/u);
  assert.match(prompt, /also cited direct output/u);
  assert.match(prompt, /Total Observation Count[\s\S]*3/u);
  assert.match(prompt, /Cited Observation Count[\s\S]*2/u);
  assert.match(prompt, /Producer-local Observation Indices/u);
  assert.match(prompt, /not valid references for the current node/u);
  assert.doesNotMatch(prompt, /unrelated artifact/u);
  assert.doesNotMatch(prompt, /uncited direct output|direct-call|root-call/u);
  assert.ok(prompt.indexOf('first body') < prompt.indexOf('second body'));
  assert.match(prompt, /Input schemas are supplied directly by the runtime/u);
  assert.doesNotMatch(prompt, /inputSchema/u);
});

test('deduplicates cross-criterion references in original observation order', () => {
  const ancestor = completedNode('ancestor', 0, []);
  ancestor.doneWhen = ['First.', 'Second.'];
  ancestor.outcome = {
    ...ancestor.outcome!,
    criteria: [
      {
        criterionIndex: 0,
        satisfied: true,
        evidence: 'First criterion.',
        observationIndices: [1],
      },
      {
        criterionIndex: 1,
        satisfied: true,
        evidence: 'Second criterion.',
        observationIndices: [0, 1],
      },
    ],
    observations: [
      observation('ancestor', 'call-0', 'first ledger output'),
      observation('ancestor', 'call-1', 'second ledger output'),
    ],
  };
  const current = createNode('current', 1, ['ancestor'], 'ready');

  const prompt = executionPrompt.user({
    request: 'Use evidence.',
    node: current,
    graph: { revision: 1, nodes: [ancestor, current] },
    skills: [],
    tools: [],
  });

  assert.equal(prompt.split('first ledger output').length - 1, 1);
  assert.equal(prompt.split('second ledger output').length - 1, 1);
  assert.ok(
    prompt.indexOf('first ledger output') <
      prompt.indexOf('second ledger output'),
  );
  assert.doesNotMatch(prompt, /call-0|call-1/u);
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
  const graph: Graph = { revision: 1, nodes: [current] };

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

test('renders artifact references as references rather than inline content', () => {
  const ancestor = completedNode('ancestor', 0, []);
  ancestor.artifacts = [
    {
      kind: 'reference',
      mime: 'application/octet-stream',
      reference: 'urn:artifact:opaque',
    },
  ];
  const current = createNode('current', 1, ['ancestor'], 'ready');
  const prompt = executionPrompt.user({
    request: 'Use the artifact.',
    node: current,
    graph: { revision: 1, nodes: [ancestor, current] },
    skills: [],
    tools: [],
  });

  assert.match(prompt, /## Kind[\s\S]*reference/u);
  assert.match(prompt, /## Reference[\s\S]*urn:artifact:opaque/u);
  assert.doesNotMatch(prompt, /## Data[\s\S]*urn:artifact:opaque/u);
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
      artifact === undefined
        ? []
        : [{ kind: 'inline', mime: 'text/plain', data: artifact }],
    outcome: null,
    termination: null,
  };
}

function completedNode(
  id: string,
  index: number,
  dependsOn: string[],
  artifact?: string,
  observationIndices: readonly number[] = [],
): Node {
  const node = createNode(id, index, dependsOn, 'completed', artifact);
  node.outcome = completedOutcome(id, observationIndices, [
    observation(id, `${id}-call`, `cited ${id} output`),
  ]);
  return node;
}

function completedOutcome(
  id: string,
  observationIndices: readonly number[],
  observations: readonly ReturnType<typeof observation>[],
) {
  return {
    status: 'completed' as const,
    criteria: [
      {
        criterionIndex: 0,
        satisfied: true,
        evidence: `${id} is complete.`,
        observationIndices: [...observationIndices],
      },
    ],
    result: { markdown: `${id} result`, artifacts: [] },
    revisionRequest: null,
    reason: null,
    observations: [...observations],
  };
}

function observation(goalId: string, callId: string, output: string) {
  return {
    goalId,
    toolName: 'inspect',
    callId,
    input: '{}',
    output,
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
