import assert from 'node:assert/strict';
import test from 'node:test';

import { AgentErrorObject } from 'agent';
import type { LlmProvider, ProviderRequest } from 'llms';

import type { PlannedGraph } from '../src/lib/schemas/graph.js';
import { revision } from '../src/lib/states/revision/index.js';
import {
  localizedRevisionCount,
  retiredNodeIds,
} from '../src/lib/states/revision/localized.js';
import type { Graph, Node } from '../src/lib/types/graph.js';
import type { MosaicOptions } from '../src/lib/types/mosaic-options.js';
import type { Observation } from '../src/lib/types/revision.js';
import type { WorkflowState } from '../src/lib/types/workflow.js';
import {
  mosaicProviders,
  terminalFinish,
  terminalTool,
  userContent,
} from './structured.js';

test('preserves completed nodes and resets retained target runtime state', async () => {
  const completed = node('completed', 0, 'completed', false);
  completed.artifacts = [
    { kind: 'inline', mime: 'text/plain', data: 'preserved result' },
  ];
  completed.candidates = [candidate('selected', 'Preserve me.')];
  completed.bundle = bundle('completed', ['selected'], 'Preserve me.');
  completed.tools = [{ name: 'lookup', description: 'Preserve me.' }];
  const target = node('target', 1, 'needs_revision', true, ['completed']);
  target.artifacts = [{ kind: 'inline', mime: 'text/plain', data: 'partial' }];
  target.candidates = [candidate('selected', 'Clear me.')];
  target.bundle = bundle('target', ['selected'], 'Clear me.');
  target.tools = [{ name: 'lookup', description: 'Clear me.' }];
  const pending = node('pending', 2, 'pending', true, ['completed']);
  const active: Graph = { revision: 1, nodes: [completed, target, pending] };
  const planned: PlannedGraph = {
    nodes: [
      plannedNode(completed),
      {
        ...plannedNode(target),
        goal: 'Revised target result',
        doneWhen: ['Revised target is complete.'],
      },
      { ...plannedNode(pending), goal: 'Changed pending result' },
    ],
  };
  requestRevision(target, [
    'context output',
    'hostile\n``````\ninvalidating output',
    'confirmation output',
  ]);
  const historicalOutcome = target.outcome;
  const harness = createHarness([planned]);
  const workflow = state(active);

  const action = await revision(
    workflow,
    { input: 'Original request.', options: harness.options },
    handlers(),
  );

  assert.equal(action.type, 'transition');
  if (action.type !== 'transition') return;
  const revised = action.state.graphs.at(-1);
  assert.ok(revised);
  assert.equal(revised.revision, 2);
  assert.deepEqual(revised.nodes[0], completed);
  assert.notStrictEqual(revised.nodes[0], completed);
  assert.notStrictEqual(revised.nodes[0]?.candidates, completed.candidates);
  assert.notStrictEqual(revised.nodes[0]?.bundle, completed.bundle);
  assert.deepEqual(revised.nodes[1]?.candidates, []);
  assert.equal(revised.nodes[1]?.bundle, null);
  assert.deepEqual(revised.nodes[1]?.tools, []);
  assert.deepEqual(revised.nodes[1]?.artifacts, []);
  assert.equal(revised.nodes[1]?.outcome, null);
  assert.equal(revised.nodes[1]?.termination, null);
  assert.strictEqual(target.outcome, historicalOutcome);
  assert.equal(target.outcome?.status, 'needs_revision');
  assert.equal(revised.nodes[1]?.status, 'pending');
  assert.equal(revised.nodes[2]?.status, 'pending');
  assert.equal(localizedRevisionCount(action.state.graphs), 1);
  assert.equal(harness.requests.length, 1);
  const request = harness.requests[0];
  assert.ok(request);
  assert.equal(request.schema, undefined);
  assert.equal(request.model, 'default');
  assert.equal(request.flags, undefined);
  assert.deepEqual(
    request.messages.map(({ role }) => role),
    ['system', 'system', 'user'],
  );
  assert.equal(request.tools?.length, 1);
  terminalTool(request);
  const prompt = userContent(request);
  assert.match(prompt, /## Revision\n\n```text\n1\n```/u);
  assert.match(prompt, /~~~text\nhostile\n`{6}\ninvalidating output/u);
  assert.ok(
    prompt.indexOf('context output') < prompt.indexOf('invalidating output'),
  );
  assert.ok(
    prompt.indexOf('invalidating output') <
      prompt.indexOf('confirmation output'),
  );
  assert.doesNotMatch(prompt, /provider-call-target/u);
  assert.doesNotMatch(prompt, /Call ID|Trigger Observation Reference/u);
  assert.doesNotMatch(prompt, /Planning Hints/u);
});

test('retires removed target and pending IDs and rejects their later reuse', async () => {
  const completed = node('completed', 0, 'completed', false);
  const target = node('target', 1, 'needs_revision', true, ['completed']);
  const pending = node('pending', 2, 'pending', true, ['completed']);
  const active: Graph = { revision: 1, nodes: [completed, target, pending] };
  const replacement = nodePlan('replacement', true, ['completed']);
  const harness = createHarness([
    { nodes: [plannedNode(completed), replacement] },
    {
      nodes: [plannedNode(completed), nodePlan('target', true, ['completed'])],
    },
  ]);
  requestRevision(target);

  const first = await revision(
    state(active),
    { input: 'Request.', options: harness.options },
    handlers(),
  );
  assert.equal(first.type, 'transition');
  if (first.type !== 'transition') return;
  assert.deepEqual(retiredNodeIds(first.state.graphs), ['target', 'pending']);

  const replacementNode = first.state.graphs.at(-1)?.nodes[1];
  assert.ok(replacementNode);
  replacementNode.status = 'needs_revision';
  requestRevision(replacementNode);
  const secondState = first.state;
  const second = await revision(
    secondState,
    { input: 'Request.', options: harness.options },
    handlers(),
  );

  assert.equal(second.type, 'fail');
  if (second.type !== 'fail') return;
  assert.match(
    (second.error as Error).message,
    /reused retired node ID target/u,
  );
  assert.equal(localizedRevisionCount(secondState.graphs), 1);
});

test('processes multiple revisions in deterministic node-wave order', async () => {
  const completed = node('completed', 0, 'completed', false);
  const firstTarget = node('first', 1, 'needs_revision', true, ['completed']);
  const secondTarget = node('second', 2, 'needs_revision', true, ['completed']);
  const active: Graph = {
    revision: 1,
    nodes: [completed, firstTarget, secondTarget],
  };
  const firstPlan: PlannedGraph = {
    nodes: [
      plannedNode(completed),
      plannedNode(firstTarget),
      { ...plannedNode(secondTarget), goal: 'Second revised' },
    ],
  };
  const secondPlan: PlannedGraph = {
    nodes: [
      plannedNode(completed),
      { ...plannedNode(firstTarget), goal: 'First revised' },
      { ...firstPlan.nodes[2], goal: 'Second may change while pending' },
    ],
  };
  requestRevision(firstTarget);
  requestRevision(secondTarget);
  const harness = createHarness([firstPlan, secondPlan]);

  const first = await revision(
    state(active),
    { input: 'Request.', options: harness.options },
    handlers(),
  );
  assert.equal(first.type, 'transition');
  if (first.type !== 'transition') return;
  assert.equal(first.state.graphs.at(-1)?.nodes[1]?.status, 'needs_revision');
  assert.equal(first.state.graphs.at(-1)?.nodes[1]?.goal, firstTarget.goal);
  assert.equal(first.state.graphs.at(-1)?.nodes[2]?.status, 'pending');
  assert.equal(first.state.graphs.at(-1)?.nodes[2]?.goal, 'Second revised');

  const second = await revision(
    first.state,
    { input: 'Request.', options: harness.options },
    handlers(),
  );
  assert.equal(second.type, 'transition');
  if (second.type !== 'transition') return;
  assert.equal(localizedRevisionCount(second.state.graphs), 2);
  assert.equal(second.state.graphs.at(-1)?.revision, 3);
  assert.equal(second.state.graphs.at(-1)?.nodes[1]?.goal, 'First revised');
  assert.match(userContent(harness.requests[0]!), /second/u);
  assert.match(userContent(harness.requests[1]!), /first/u);
});

test('blocks without a provider call when the localized revision limit is zero or exhausted', async () => {
  for (const { max, count } of [
    { max: 0, count: 0 },
    { max: 1, count: 1 },
  ]) {
    const target = node('target', 0, 'needs_revision', true);
    const active: Graph = { revision: 1, nodes: [target] };
    const harness = createHarness([], max);
    requestRevision(target);
    const workflow = state(active, count);

    const action = await revision(
      workflow,
      { input: 'Request.', options: harness.options },
      handlers(),
    );

    assert.equal(action.type, 'transition');
    assert.equal(target.status, 'blocked');
    assert.deepEqual(target.termination, {
      type: 'revision_limit',
      status: 'blocked',
      limit: max,
    });
    assert.equal(target.outcome?.status, 'needs_revision');
    assert.equal(harness.requests.length, 0);
    assert.equal(localizedRevisionCount(workflow.graphs), count);
  }
});

test('derives localized revision consumption from the active revision field', () => {
  const active: Graph = {
    revision: 7,
    nodes: [node('active', 0, 'pending', true)],
  };
  assert.equal(localizedRevisionCount([active]), 6);
});

test('rejects changes to a completed node without consuming the revision', async () => {
  const completed = node('completed', 0, 'completed', false);
  const target = node('target', 1, 'needs_revision', true, ['completed']);
  const active: Graph = { revision: 1, nodes: [completed, target] };
  const invalid: PlannedGraph = {
    nodes: [
      { ...plannedNode(completed), goal: 'Changed completed result' },
      plannedNode(target),
    ],
  };
  requestRevision(target);
  const workflow = state(active);
  const harness = createHarness([invalid]);

  const action = await revision(
    workflow,
    { input: 'Request.', options: harness.options },
    handlers(),
  );

  assert.equal(action.type, 'fail');
  assert.equal(localizedRevisionCount(workflow.graphs), 0);
  assert.equal(target.outcome?.revisionRequest?.goalId, 'target');
  assert.deepEqual(completed.artifacts, []);
});

test('validates the active graph and target request before calling the provider', async () => {
  const invalidGraphTarget = node('invalid-graph', 0, 'needs_revision', true);
  requestRevision(invalidGraphTarget);
  invalidGraphTarget.goal = ' ';

  const invalidRequestTarget = node(
    'invalid-request',
    0,
    'needs_revision',
    true,
  );
  requestRevision(invalidRequestTarget);
  assert.ok(invalidRequestTarget.outcome?.revisionRequest);
  invalidRequestTarget.outcome.revisionRequest.goalId = 'another-node';

  for (const target of [invalidGraphTarget, invalidRequestTarget]) {
    const workflow = state({ revision: 1, nodes: [target] });
    const harness = createHarness([]);

    const action = await revision(
      workflow,
      { input: 'Request.', options: harness.options },
      handlers(),
    );

    assert.equal(action.type, 'fail');
    assert.equal(harness.requests.length, 0);
    assert.equal(workflow.graphs.length, 2);
  }
});

test('does not append a graph when the provider fails or returns an invalid plan', async () => {
  const failure = new Error('provider unavailable');
  const cases = [
    { plans: [] as unknown[], failure },
    { plans: [{ nodes: [] }] as unknown[], failure: undefined },
  ];

  for (const input of cases) {
    const target = node('target', 0, 'needs_revision', true);
    requestRevision(target);
    const workflow = state({ revision: 1, nodes: [target] });
    const harness = createHarness(input.plans, 3, input.failure);

    const action = await revision(
      workflow,
      { input: 'Request.', options: harness.options },
      handlers(),
    );

    assert.equal(action.type, 'fail');
    assert.equal(harness.requests.length, input.failure === undefined ? 3 : 1);
    if (input.failure === undefined && action.type === 'fail') {
      assert.ok(action.error instanceof AgentErrorObject);
      assert.equal(action.error.data.code, 'invalid_structured_output');
    }
    assert.equal(workflow.graphs.length, 2);
    assert.equal(target.status, 'needs_revision');
    assert.equal(target.outcome?.revisionRequest?.goalId, 'target');
  }
});

const createHarness = (plans: readonly unknown[], max = 3, failure?: Error) => {
  let index = 0;
  const requests: ProviderRequest<unknown>[] = [];
  const options: MosaicOptions = {
    logger: { info: () => undefined, debug: () => undefined } as never,
    providers: mosaicProviders({
      metadata: {
        id: 'fake',
        name: 'Fake',
        baseUrl: 'https://fake.invalid',
      },
      complete: async (request: ProviderRequest<unknown>) => {
        requests.push(request);
        if (failure !== undefined) throw failure;
        terminalTool(request);
        const plan = plans[index++] ?? plans.at(-1);
        return terminalFinish(request, plan);
      },
    } as unknown as LlmProvider),
    models: {
      planning: { model: 'default', effort: 'low' },
      revision: { model: 'default', effort: 'low' },
      execution: { model: 'default', effort: 'low' },
      reranker: 'reranker',
      embedder: 'embedder',
    },
    routing: {
      maxHintCandidates: 1,
      maxRetrievedCandidates: 1,
      maxSkills: 0,
    },
    execution: { maxTurns: 8 },
    revision: { max },
    skills: {
      required: [],
      menu: [],
      retriever: new Proxy({} as never, {
        get() {
          throw new Error('Localized revision must not query skill hints.');
        },
      }),
    },
    tools: { required: [], menu: [], retriever: {} as never },
  };

  return { options, requests };
};

const state = (active: Graph, localizedCount = 0): WorkflowState => ({
  graphs: Array.from({ length: localizedCount + 2 }, (_, revision) => ({
    ...active,
    revision,
  })),
});

const requestRevision = (
  target: Node,
  outputs: readonly string[] = ['observed output'],
): void => {
  const goalId = target.id;
  const observations: Observation[] = outputs.map((output, index) => ({
    goalId,
    toolName: `lookup-${index + 1}`,
    callId: `provider-call-${goalId}-${index + 1}`,
    input: `{"query":"evidence-${index + 1}"}`,
    output,
  }));
  target.status = 'needs_revision';
  target.outcome = {
    status: 'needs_revision',
    criteria: [
      {
        criterionIndex: 0,
        satisfied: false,
        evidence: 'Not complete.',
        observationIndices: [],
      },
    ],
    result: null,
    revisionRequest: {
      goalId,
      invalidatedAssumption: 'The original structure remains valid.',
      requestedEffect: 'Revise the target structure.',
    },
    reason: 'Revision required.',
    observations,
  };
  target.termination = null;
};

const node = (
  id: string,
  index: number,
  status: Node['status'],
  deliver: boolean,
  dependsOn: string[] = [],
): Node => ({
  ...nodePlan(id, deliver, dependsOn),
  status,
  index,
  candidates: [],
  bundle: null,
  tools: [],
  artifacts: [],
  outcome: status === 'completed' ? completedOutcome(id) : null,
  termination: null,
});

const completedOutcome = (id: string) => ({
  status: 'completed' as const,
  criteria: [
    {
      criterionIndex: 0,
      satisfied: true,
      evidence: `${id} complete.`,
      observationIndices: [],
    },
  ],
  result: { markdown: `${id} result`, artifacts: [] },
  revisionRequest: null,
  reason: null,
  observations: [],
});

const candidate = (skillName: string, rationale: string) => ({
  skillName,
  score: 1,
  rank: 1,
  rationale,
});

const bundle = (
  goalId: string,
  skills: readonly string[],
  selectionRationale: string,
) => ({ goalId, skills: [...skills], selectionRationale });

const nodePlan = (
  id: string,
  deliver: boolean,
  dependsOn: string[] = [],
): PlannedGraph['nodes'][number] => ({
  id,
  goal: `Goal ${id}`,
  doneWhen: [`${id} is complete.`],
  dependsOn,
  deliver,
});

const plannedNode = (value: Node): PlannedGraph['nodes'][number] => ({
  id: value.id,
  goal: value.goal,
  doneWhen: [...value.doneWhen],
  dependsOn: [...value.dependsOn],
  deliver: value.deliver,
});

const handlers = () =>
  ({
    transition: (handler: string, value: WorkflowState) => ({
      type: 'transition' as const,
      handler,
      state: value,
    }),
    fail: (error: unknown) => ({ type: 'fail' as const, error }),
  }) as never;
