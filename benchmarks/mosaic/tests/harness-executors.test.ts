import assert from 'node:assert/strict';
import test from 'node:test';
import {
  ProviderErrorObject,
  type LlmProvider,
  type ProviderRequest,
} from 'llms';
import type { RoutingInput } from 'mosaic/evaluation';

import { SKILLS } from '../src/catalog/index.js';
import {
  A1,
  A2,
  A4,
  B0,
  B1,
  B2,
  B3,
  M1,
  createProviderBaselineModel,
  createTestLexicalRetrievers,
  evaluationHooks,
  executeBaseline,
  executeMosaic,
  loadConditionPrompts,
  mosaicSkills,
  type BaselineModel,
  type MosaicDependencies,
} from '../src/conditions/index.js';
import { BASE_TOOL_NAMES } from '../src/config/index.js';
import { artifactHash, type HarnessJsonValue } from '../src/core/index.js';
import {
  TOOL_CONTRACTS,
  HarnessInfrastructureError,
  createWorld,
  executeTool,
  type EngineUsage,
  type RunContext,
} from '../src/runtime/index.js';
import { PILOT_CASES, createSchedule } from '../src/study/index.js';

const usage = { inputTokens: 1, outputTokens: 1, costUsd: 0.01 } as const;
const plan = {
  goals: [
    {
      id: 'g01',
      goal: 'Reconcile invoice evidence.',
      doneWhen: ['The result cites the fixture.'],
    },
  ],
} as const;

const contextFor = (condition: typeof B0) => {
  const benchmarkCase = PILOT_CASES[0]!;
  const run = createSchedule({
    studyId: `executor.${condition.id}`,
    cases: [benchmarkCase],
    conditions: [condition],
    seed: condition.id,
    repetitions: 1,
  })[0]!;
  const events: HarnessJsonValue[] = [];
  let world = createWorld();
  let modelCalls = 0;
  const context: RunContext = {
    run,
    benchmarkCase,
    condition,
    policy: {} as never,
    emit: async (event) => {
      events.push(event);
    },
    startModelCall: async () => {
      modelCalls += 1;
    },
    callTool: async (name, input, correlation) => {
      const callId = correlation?.callId ?? 'test-call';
      await context.emit({
        type: 'tool.call.started',
        name,
        callId,
        ...(correlation?.nodeId === undefined
          ? {}
          : { nodeId: correlation.nodeId }),
        ...(correlation?.revision === undefined
          ? {}
          : { revision: correlation.revision }),
        input,
      });
      const result = executeTool(world, name, input);
      world = result.world;
      await context.emit({
        type: 'tool.call.finished',
        name,
        callId,
        ...(correlation?.nodeId === undefined
          ? {}
          : { nodeId: correlation.nodeId }),
        ...(correlation?.revision === undefined
          ? {}
          : { revision: correlation.revision }),
        output: result.output,
        evidenceHash: artifactHash({ name, input, output: result.output }),
      });
      return result.output;
    },
    world: () => world,
  };
  return { context, events, modelCalls: () => modelCalls };
};

const terminalModel = (): BaselineModel => ({
  plan: async () => ({ value: plan, usage }),
  execute: async () => ({
    value: { status: 'completed', output: 'done', toolCalls: [] },
    usage,
  }),
  revise: async () => ({ value: plan.goals[0], usage }),
});

test('baseline conditions expose the frozen skill and tool menus', async () => {
  const prompts = await loadConditionPrompts();
  for (const condition of [B0, B1, B2, B3]) {
    const fixture = contextFor(condition);
    await executeBaseline(fixture.context, terminalModel(), prompts);
    const menu = fixture.events.find(
      (event) =>
        typeof event === 'object' &&
        event !== null &&
        !Array.isArray(event) &&
        (event as Readonly<Record<string, HarnessJsonValue>>)['type'] ===
          'baseline.menu',
    ) as Readonly<Record<string, HarnessJsonValue>>;
    const skillNames = menu['skillNames'] as readonly string[];
    const toolNames = menu['toolNames'] as readonly string[];
    if (condition.id === 'B0')
      assert.deepEqual([skillNames.length, toolNames.length], [0, 24]);
    if (condition.id === 'B1')
      assert.deepEqual([skillNames.length, toolNames.length], [1, 24]);
    if (condition.id === 'B2') {
      assert.equal(skillNames.length, 1);
      assert.deepEqual(
        toolNames,
        SKILLS.find((skill) => skill.id === skillNames[0])?.allowedTools,
      );
    }
    if (condition.id === 'B3') {
      assert.equal(skillNames.length, 3);
      assert.ok(BASE_TOOL_NAMES.every((name) => toolNames.includes(name)));
    }
  }
});

test('baseline tool loop returns observations to the next model turn', async () => {
  const prompts = await loadConditionPrompts();
  const fixture = contextFor(B0);
  const users: string[] = [];
  let turn = 0;
  const model: BaselineModel = {
    plan: async () => ({ value: plan, usage }),
    revise: async () => ({ value: plan.goals[0], usage }),
    execute: async (request) => {
      users.push(request.user);
      turn += 1;
      return turn === 1
        ? {
            value: {
              status: 'completed',
              output: '',
              toolCalls: [
                { name: 'read', input: { path: 'notes/request.md' } },
              ],
            },
            usage,
          }
        : {
            value: {
              status: 'completed',
              output: 'world-v1 observed',
              toolCalls: [],
            },
            usage,
          };
    },
  };
  const result = await executeBaseline(fixture.context, model, prompts);
  assert.equal(fixture.modelCalls(), 2);
  assert.equal(result.usage.inputTokens, 2);
  assert.equal(users[0]?.includes('Pilot request evidence'), false);
  assert.doesNotMatch(
    users[0]!,
    /Uses only the required deterministic evidence operations/u,
  );
  assert.match(
    users[1]!,
    /# Tool observations[\s\S]*Pilot request evidence for world-v1/u,
  );
  assert.equal(prompts.baseline, (await loadConditionPrompts()).baseline);
});

test('baseline planning derives criteria only from the visible request', async () => {
  const prompts = await loadConditionPrompts();
  const fixture = contextFor(B2);
  let planningUser = '';
  const model: BaselineModel = {
    plan: async (request) => {
      planningUser = request.user;
      return { value: plan, usage };
    },
    execute: async () => ({
      value: { status: 'completed', output: 'done', toolCalls: [] },
      usage,
    }),
    revise: async () => ({ value: plan.goals[0], usage }),
  };
  await executeBaseline(fixture.context, model, prompts);
  assert.match(planningUser, /# Request/u);
  assert.match(planningUser, /Derive observable completion criteria only/u);
  assert.doesNotMatch(
    planningUser,
    /Uses only the required deterministic evidence operations/u,
  );
});

test('baseline model failures emit a sanitized terminal model event', async () => {
  const prompts = await loadConditionPrompts();
  const fixture = contextFor(B0);
  const model: BaselineModel = {
    plan: async () => ({ value: plan, usage }),
    revise: async () => ({ value: plan.goals[0], usage }),
    execute: async () => {
      throw new Error('credential-bearing provider failure');
    },
  };
  await assert.rejects(
    executeBaseline(fixture.context, model, prompts),
    /credential-bearing provider failure/u,
  );
  const serialized = JSON.stringify(fixture.events);
  assert.match(serialized, /model\.failed/u);
  assert.doesNotMatch(serialized, /credential-bearing provider failure/u);
});

test('baseline records a rejected structured attempt before translating a provider failure', async () => {
  const prompts = await loadConditionPrompts();
  const fixture = contextFor(B0);
  const failure = new ProviderErrorObject({
    provider: 'fake',
    code: 'invalid_structured_output',
    message: 'private rejected payload and credential',
    diagnostic: 'raw private provider diagnostic',
  });
  const provider = {
    complete: async () => {
      throw failure;
    },
  } as unknown as LlmProvider;
  const model = createProviderBaselineModel({
    provider,
    model: 'openai/gpt-5.6-luna',
  });

  await assert.rejects(
    executeBaseline(fixture.context, model, prompts),
    (error: unknown) =>
      error instanceof HarnessInfrastructureError &&
      error.stage === 'model' &&
      error.code === 'provider_failed',
  );
  const lifecycle = fixture.events.flatMap((event) => {
    if (typeof event !== 'object' || event === null || Array.isArray(event))
      return [];
    const payload = event as Readonly<Record<string, HarnessJsonValue>>;
    return payload['type'] === 'structured.attempt' ||
      payload['type'] === 'model.failed'
      ? [payload]
      : [];
  });
  assert.deepEqual(lifecycle, [
    {
      type: 'structured.attempt',
      stage: 'execution',
      attempt: 1,
      runtimeAccepted: false,
      feedbackSent: false,
      diagnostic: 'invalid_structured_output',
    },
    {
      type: 'model.failed',
      stage: 'execution',
      operation: 'execute',
      model: 'openai/gpt-5.6-luna',
      status: 'failed',
      durationMs: lifecycle[1]?.['durationMs'],
    },
  ]);
  const serialized = JSON.stringify(fixture.events);
  assert.doesNotMatch(
    serialized,
    /private rejected payload|credential|raw private/u,
  );
});

test('B3 includes localized-revision usage and re-execution', async () => {
  const prompts = await loadConditionPrompts();
  const fixture = contextFor(B3);
  let execution = 0;
  const model: BaselineModel = {
    plan: async () => ({ value: plan, usage }),
    execute: async () => {
      execution += 1;
      return execution === 1
        ? {
            value: {
              status: 'needs_revision',
              output: '',
              toolCalls: [],
              revisionReason: 'fixture changed',
            },
            usage,
          }
        : {
            value: { status: 'completed', output: 'done', toolCalls: [] },
            usage,
          };
    },
    revise: async () => ({
      value: { ...plan.goals[0], goal: 'Revised goal' },
      usage: { inputTokens: 5, outputTokens: 5, costUsd: 0.05 },
    }),
  };
  const result = await executeBaseline(fixture.context, model, prompts);
  assert.equal(fixture.modelCalls(), 4);
  assert.equal(result.usage.inputTokens, 8);
  assert.equal(result.usage.costUsd, 0.08);
});

test('A2 preserves selection through next while A4 bypasses selector', async () => {
  const benchmarkCase = PILOT_CASES[0]!;
  const trace = {
    candidates: ['a', 'b', 'c'].map((skillName, index) => ({
      skillName,
      score: 3 - index,
      rank: index + 1,
      rationale: 'selected',
    })),
    bundle: {
      goalId: 'g01',
      skills: ['a', 'b', 'c'],
      selectionRationale: 'selected',
    },
  };
  const input = {
    request: 'request',
    graph: { revision: 1 },
    node: { id: 'g01', goal: 'goal', doneWhen: ['done'] },
  } as unknown as RoutingInput;
  let a2Next = 0;
  const a2 = evaluationHooks(A2, benchmarkCase, 17).routing!;
  const shuffled = await a2(input, async () => {
    a2Next += 1;
    return trace;
  });
  assert.equal(a2Next, 1);
  assert.deepEqual(
    new Set(shuffled.bundle.skills),
    new Set(trace.bundle.skills),
  );

  const skills = mosaicSkills();
  let reranks = 0;
  let modelCalls = 0;
  const dependencies = {
    base: {
      logger: {} as never,
      provider: {
        rerank: async () => {
          reranks += 1;
          return skills
            .slice(0, 4)
            .map((_, index) => ({ index, relevanceScore: 1 - index / 10 }));
        },
      } as never,
      models: {
        planning: { model: 'default', effort: 'medium' },
        revision: { model: 'default', effort: 'medium' },
        execution: { model: 'default', effort: 'medium' },
        reranker: 'reranker',
        embedder: 'embedder',
      },
      routing: {
        maxHintCandidates: 3,
        maxRetrievedCandidates: 4,
        maxSkills: 3,
      },
      execution: { maxTurns: 8 },
      revision: { max: 3 },
    },
    retrievers: {
      skills: {
        search: async () =>
          skills
            .slice(0, 4)
            .map((data, index) => ({ data, score: 1 - index / 10 })),
      },
      metadataSkills: createTestLexicalRetrievers().metadataSkills,
      tools: createTestLexicalRetrievers().tools,
    },
  } satisfies MosaicDependencies;
  let a4Next = 0;
  const a4 = evaluationHooks(
    A4,
    benchmarkCase,
    17,
    {},
    dependencies,
    async () => {
      modelCalls += 1;
    },
  ).routing!;
  const fixed = await a4(input, async () => {
    a4Next += 1;
    return trace;
  });
  assert.equal(a4Next, 0);
  assert.equal(reranks, 1);
  assert.equal(modelCalls, 1);
  assert.deepEqual(
    fixed.bundle.skills,
    skills.slice(0, 3).map((skill) => skill.name),
  );
});

test('A1 uses the observed metadata retriever for hints and bundle retrieval', async () => {
  const fixture = contextFor(A1);
  const skills = mosaicSkills();
  let bodySearches = 0;
  let metadataSearches = 0;
  const dependencies = {
    base: {
      logger: {} as never,
      provider: {} as never,
      models: {
        planning: { model: 'default', effort: 'medium' },
        revision: { model: 'default', effort: 'medium' },
        execution: { model: 'default', effort: 'medium' },
        reranker: 'reranker',
        embedder: 'embedder',
      },
      routing: {
        maxHintCandidates: 3,
        maxRetrievedCandidates: 4,
        maxSkills: 3,
      },
      execution: { maxTurns: 8 },
      revision: { max: 3 },
    },
    retrievers: {
      skills: {
        search: async () => {
          bodySearches += 1;
          return skills.slice(0, 3).map((data) => ({ data, score: 1 }));
        },
      },
      metadataSkills: {
        search: async () => {
          metadataSearches += 1;
          return skills.slice(0, 3).map((data) => ({ data, score: 1 }));
        },
      },
      tools: createTestLexicalRetrievers().tools,
    },
    retrieverModels: {
      skills: 'voyageai/voyage-4-large',
      metadataSkills: 'voyageai/voyage-4-large',
    },
    factory: ((
      options: Parameters<NonNullable<MosaicDependencies['factory']>>[0],
      evaluation: Parameters<NonNullable<MosaicDependencies['factory']>>[1],
    ) => ({
      prompt: async () => {
        await options.skills.retriever.search('hint query', 3);
        const hook = evaluation?.hooks?.retrieval;
        if (hook !== undefined) {
          await hook(
            { query: 'bundle query', limit: 3, catalog: skills } as never,
            async () => [],
          );
        }
        return { status: 'failed', nodes: [] };
      },
    })) as NonNullable<MosaicDependencies['factory']>,
  } satisfies MosaicDependencies;
  await executeMosaic(fixture.context, dependencies);
  assert.equal(bodySearches, 0);
  assert.equal(metadataSearches, 2);
  assert.equal(fixture.modelCalls(), 2);
  const embeddingEvents = fixture.events.filter(
    (event) =>
      typeof event === 'object' &&
      event !== null &&
      !Array.isArray(event) &&
      (event as Readonly<Record<string, HarnessJsonValue>>)['operation'] ===
        'embedding',
  );
  assert.equal(
    embeddingEvents.filter(
      (event) =>
        (event as Readonly<Record<string, HarnessJsonValue>>)['type'] ===
        'model.request',
    ).length,
    2,
  );
  assert.equal(
    embeddingEvents.filter(
      (event) =>
        (event as Readonly<Record<string, HarnessJsonValue>>)['type'] ===
        'model.response',
    ).length,
    2,
  );
  assert.ok(
    embeddingEvents
      .filter(
        (event) =>
          (event as Readonly<Record<string, HarnessJsonValue>>)['type'] ===
          'model.response',
      )
      .every(
        (event) =>
          (event as Readonly<Record<string, HarnessJsonValue>>)[
            'resultCount'
          ] === 3,
      ),
  );
});

test('MOSAIC correlates concurrent same-tool calls with their node revision and call ID', async () => {
  const fixture = contextFor(A1);
  const retrievers = createTestLexicalRetrievers();
  const dependencies = {
    base: {
      logger: {} as never,
      provider: {} as never,
      models: {
        planning: { model: 'default', effort: 'medium' },
        revision: { model: 'default', effort: 'medium' },
        execution: { model: 'default', effort: 'medium' },
        reranker: 'reranker',
        embedder: 'embedder',
      },
      routing: {
        maxHintCandidates: 3,
        maxRetrievedCandidates: 5,
        maxSkills: 3,
      },
      execution: { maxTurns: 8 },
      revision: { max: 3 },
    },
    retrievers,
    factory: ((
      options: Parameters<NonNullable<MosaicDependencies['factory']>>[0],
    ) => ({
      prompt: async (
        _input: string,
        runOptions?: Parameters<
          ReturnType<NonNullable<MosaicDependencies['factory']>>['prompt']
        >[1],
      ) => {
        const read = options.tools.menu.find(({ name }) => name === 'read')!;
        const invoke = async (nodeId: string, callId: string) => {
          await runOptions?.observer?.({
            schemaVersion: 1,
            runId: 'mosaic-tool-correlation',
            sequence: callId === 'call-a' ? 1 : 2,
            type: 'tool.started',
            stage: 'execution',
            nodeId,
            revision: 1,
            callId,
            toolName: 'read',
          });
          await read.execute({ path: 'notes/request.md' });
          await runOptions?.observer?.({
            schemaVersion: 1,
            runId: 'mosaic-tool-correlation',
            sequence: callId === 'call-a' ? 3 : 4,
            type: 'tool.finished',
            stage: 'execution',
            nodeId,
            revision: 1,
            callId,
            toolName: 'read',
            durationMs: 0,
          });
        };
        await Promise.all([
          invoke('node-a', 'call-a'),
          invoke('node-b', 'call-b'),
        ]);
        return { status: 'failed', nodes: [] };
      },
    })) as NonNullable<MosaicDependencies['factory']>,
  } satisfies MosaicDependencies;
  await executeMosaic(fixture.context, dependencies);
  const starts = fixture.events.flatMap((event) => {
    if (typeof event !== 'object' || event === null || Array.isArray(event))
      return [];
    const payload = event as Readonly<Record<string, HarnessJsonValue>>;
    return payload['type'] === 'tool.call.started' ? [payload] : [];
  });
  assert.deepEqual(
    starts.map(({ callId, nodeId, revision }) => ({
      callId,
      nodeId,
      revision,
    })),
    [
      { callId: 'call-a', nodeId: 'node-a', revision: 1 },
      { callId: 'call-b', nodeId: 'node-b', revision: 1 },
    ],
  );
});

test('model-backed retrieval emits a sanitized terminal failure event', async () => {
  const fixture = contextFor(A1);
  const dependencies = {
    base: {
      logger: {} as never,
      provider: {} as never,
      models: {
        planning: { model: 'default', effort: 'medium' },
        revision: { model: 'default', effort: 'medium' },
        execution: { model: 'default', effort: 'medium' },
        reranker: 'reranker',
        embedder: 'embedder',
      },
      routing: {
        maxHintCandidates: 3,
        maxRetrievedCandidates: 4,
        maxSkills: 3,
      },
      execution: { maxTurns: 8 },
      revision: { max: 3 },
    },
    retrievers: {
      skills: createTestLexicalRetrievers().skills,
      metadataSkills: {
        search: async (): Promise<never> => {
          throw new Error('provider credential leaked');
        },
      },
      tools: createTestLexicalRetrievers().tools,
    },
    retrieverModels: { metadataSkills: 'voyageai/voyage-4-large' },
    factory: ((
      options: Parameters<NonNullable<MosaicDependencies['factory']>>[0],
    ) => ({
      prompt: async () => {
        await options.skills.retriever.search('private retrieval query', 3);
        return { status: 'failed', nodes: [] };
      },
    })) as NonNullable<MosaicDependencies['factory']>,
  } satisfies MosaicDependencies;
  await assert.rejects(
    executeMosaic(fixture.context, dependencies),
    /provider credential leaked/u,
  );
  const serialized = JSON.stringify(fixture.events);
  assert.doesNotMatch(serialized, /provider credential leaked/u);
  assert.match(serialized, /model\.failed/u);
  assert.doesNotMatch(serialized, /model\.response/u);
});

test('tool descriptors expose distinct operational schemas to MOSAIC', () => {
  assert.equal(
    TOOL_CONTRACTS.read.input.safeParse({ path: 'notes/request.md' }).success,
    true,
  );
  assert.equal(
    TOOL_CONTRACTS.read.input.safeParse({ query: 'wrong' }).success,
    false,
  );
  assert.equal(
    TOOL_CONTRACTS.currency_convert.input.safeParse({
      from: 'USD',
      to: 'BRL',
      amount: 2,
    }).success,
    true,
  );
  assert.equal(
    new Set(
      Object.values(TOOL_CONTRACTS).map((contract) => contract.description),
    ).size,
    24,
  );
});

test('provider baseline adapter fixes medium effort and preserves Markdown prompts', async () => {
  const requests: Record<string, unknown>[] = [];
  const metered: EngineUsage[] = [];
  const provider = {
    complete: async (request: Record<string, unknown>) => {
      requests.push(request);
      return {
        structured: { status: 'completed', output: 'done', toolCalls: [] },
        usage: { inputTokens: 3, outputTokens: 2 },
      };
    },
  } as never;
  const model = createProviderBaselineModel({
    provider,
    model: 'openai/gpt-5.6-luna',
    onUsage: (value) => {
      metered.push(value);
    },
  });
  const answer = await model.execute({
    system: '# System',
    user: '# Request\n\nHuman-readable.',
    toolNames: [],
  });
  assert.equal(requests[0]?.['effort'], 'medium');
  assert.match(
    (requests[0]?.['messages'] as { content: string }[])[1]?.content!,
    /^# Request/u,
  );
  assert.deepEqual(answer.usage, {
    inputTokens: 3,
    outputTokens: 2,
    cachedInputTokens: undefined,
    reasoningTokens: undefined,
    costUsd: 0,
  });
  assert.deepEqual(metered, [answer.usage]);
});

test('real MOSAIC agent prompt translates provider failures without exposing diagnostics', async () => {
  const fixture = contextFor(M1);
  const failure = new ProviderErrorObject({
    provider: 'fake',
    code: 'upstream_failed',
    message: 'private upstream provider failure',
    diagnostic: 'credential-bearing upstream diagnostic',
  });
  const provider = {
    metadata: {
      id: 'fake',
      name: 'Fake',
      baseUrl: 'https://fake.invalid',
    },
    complete: async (_request: ProviderRequest<unknown>) => {
      throw failure;
    },
  } as unknown as LlmProvider;
  const dependencies = {
    base: {
      logger: { info: () => undefined, debug: () => undefined } as never,
      provider,
      models: {
        planning: { model: 'default', effort: 'medium' },
        revision: { model: 'default', effort: 'medium' },
        execution: { model: 'default', effort: 'medium' },
        reranker: 'reranker',
        embedder: 'embedder',
      },
      routing: {
        maxHintCandidates: 3,
        maxRetrievedCandidates: 4,
        maxSkills: 3,
      },
      execution: { maxTurns: 8 },
      revision: { max: 3 },
    },
    retrievers: createTestLexicalRetrievers(),
  } satisfies MosaicDependencies;

  await assert.rejects(
    executeMosaic(fixture.context, dependencies),
    (error: unknown) =>
      error instanceof HarnessInfrastructureError &&
      error.stage === 'model' &&
      error.code === 'provider_failed',
  );
  assert.doesNotMatch(
    JSON.stringify(fixture.events),
    /private upstream|credential-bearing/u,
  );
});
