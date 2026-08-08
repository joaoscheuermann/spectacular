import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import {
  B0,
  B3,
  M1,
  conditionById,
  createTestLexicalRetrievers,
  evaluationHooks,
  executeBaseline,
  executeMosaic,
  loadConditionPrompts,
  type BaselineModel,
  type MosaicDependencies,
} from '../src/conditions/index.js';
import { TOOL_NAMES } from '../src/config/index.js';
import type { HarnessJsonValue } from '../src/core/index.js';
import {
  HarnessInfrastructureError,
  createEventStore,
  createRecordStore,
  executeRun,
  type CrashBoundary,
} from '../src/runtime/index.js';
import {
  PILOT_CASES,
  createSchedule,
  evaluateEvidence,
  resumeAction,
  toScoreEvidence,
} from '../src/study/index.js';

const temporary = () => mkdtemp(join(tmpdir(), 'mosaic-harness-test-'));

test('event store validates IDs and serializes concurrent content-addressed appends', async () => {
  const directory = await temporary();
  try {
    const store = createEventStore(directory);
    await assert.rejects(store.append('../escape', 1, { value: 1 }));
    await assert.rejects(store.append('run.concurrent', 0, { value: 1 }));
    await Promise.all(
      Array.from({ length: 8 }, (_, value) =>
        store.append('run.concurrent', 1, { value }),
      ),
    );
    const events = await store.read('run.concurrent', 1);
    assert.deepEqual(
      events.map((event) => event.sequence),
      [1, 2, 3, 4, 5, 6, 7, 8],
    );
    assert.ok(events.every((event) => event.attempt === 1));
    assert.ok(
      events
        .slice(1)
        .every(
          (event, index) => event.previousHash === events[index]?.eventHash,
        ),
    );
    assert.deepEqual(
      await store.derive('run.concurrent', 1),
      await store.derive('run.concurrent', 1),
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test('structure capture removes model and tool IO while preserving semantic events', async () => {
  const directory = await temporary();
  try {
    const events = createEventStore(directory);
    const records = createRecordStore(directory);
    const run = createSchedule({
      studyId: 'capture',
      cases: PILOT_CASES.slice(0, 1),
      conditions: [M1],
      seed: 'capture',
      repetitions: 1,
    })[0]!;
    const result = await executeRun(run, PILOT_CASES[0]!, M1, {
      events,
      records,
      execute: async (context) => {
        await context.emit({
          type: 'semantic.graph',
          graph: { revision: 1 },
          prompt: 'private prompt',
          reasoning: 'private reasoning',
        });
        await context.emit({
          type: 'structured.attempt',
          stage: 'plan',
          attempt: 1,
          runtimeAccepted: false,
          feedbackSent: true,
          diagnostic: 'schema_mismatch',
        });
        await context.startModelCall();
        await context.callTool('read', { path: 'notes/request.md' });
        return {
          status: 'succeeded',
          outcome: { ok: true },
          usage: { inputTokens: 2, outputTokens: 1, costUsd: 0.01 },
        };
      },
    });
    assert.equal(result.status, 'succeeded');
    const serialized = JSON.stringify(
      await events.read(run.id, result.attempt),
    );
    assert.doesNotMatch(
      serialized,
      /private prompt|private reasoning|notes\/request|Pilot request evidence/u,
    );
    assert.match(serialized, /semantic\.graph/u);
    assert.match(serialized, /schema_mismatch/u);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test('baseline model events expose only structural metadata unless IO capture is requested', async () => {
  const directory = await temporary();
  try {
    const benchmarkCase = PILOT_CASES[6]!;
    const prompts = await loadConditionPrompts();
    const marker = benchmarkCase.gold.expectedDelivery.contains.join(' ');
    const model: BaselineModel = {
      plan: async () => ({
        value: { goals: [] },
        usage: { inputTokens: 0, outputTokens: 0, costUsd: 0 },
      }),
      revise: async () => ({
        value: { id: 'unused', goal: 'unused', doneWhen: ['unused'] },
        usage: { inputTokens: 0, outputTokens: 0, costUsd: 0 },
      }),
      execute: async () => ({
        value: { status: 'completed', output: marker, toolCalls: [] },
        usage: { inputTokens: 1, outputTokens: 1, costUsd: 0 },
      }),
    };
    const captures = await Promise.all(
      (['structure', 'io'] as const).map(async (capture) => {
        const events = createEventStore(directory);
        const records = createRecordStore(directory);
        const run = createSchedule({
          studyId: `baseline.capture.${capture}`,
          cases: [benchmarkCase],
          conditions: [B0],
          seed: capture,
          repetitions: 1,
          capture,
        })[0]!;
        const record = await executeRun(run, benchmarkCase, B0, {
          events,
          records,
          execute: (context) => executeBaseline(context, model, prompts),
        });
        return JSON.stringify(
          (await events.read(run.id, record.attempt)).map(
            ({ payload }) => payload,
          ),
        );
      }),
    );
    const [structure, io] = captures;
    assert.match(structure, /model\.request/u);
    assert.match(structure, /model\.response/u);
    assert.match(structure, /structured\.attempt/u);
    assert.match(structure, /durationMs/u);
    assert.doesNotMatch(structure, /# Request|pilot-07-complete/u);
    assert.match(io, /# Request/u);
    assert.match(io, /pilot-07-complete/u);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test('io capture removes case-variant credential and private-reasoning keys', async () => {
  const directory = await temporary();
  try {
    const events = createEventStore(directory);
    const records = createRecordStore(directory);
    const run = createSchedule({
      studyId: 'capture.io',
      cases: PILOT_CASES.slice(0, 1),
      conditions: [M1],
      seed: 'capture-io',
      repetitions: 1,
      capture: 'io',
    })[0]!;
    await executeRun(run, PILOT_CASES[0]!, M1, {
      events,
      records,
      execute: async (context) => {
        await context.emit({
          type: 'custom.io',
          content: {
            safe: 'visible',
            apiKey: 'api-secret',
            ACCESS_TOKEN: 'access-secret',
            auth: 'auth-secret',
            Bearer: 'bearer-secret',
            Key: 'key-secret-exact',
            session_token: 'session-secret',
            refreshToken: 'refresh-secret',
            Credentials: 'credential-secret',
            replay: [{ type: 'reasoning', encrypted_content: 'ciphertext' }],
            nested: {
              private_key: 'key-secret',
              reasoning: 'hidden thought',
              safeNested: 'visible-too',
            },
          },
        });
        return {
          status: 'succeeded',
          outcome: { ok: true },
          usage: { inputTokens: 0, outputTokens: 0, costUsd: 0 },
        };
      },
    });
    const attempts = await records.read(run.id);
    const serialized = JSON.stringify(
      await events.read(run.id, attempts[0]!.attempt),
    );
    assert.match(serialized, /visible|visible-too/u);
    assert.doesNotMatch(
      serialized,
      /api-secret|access-secret|auth-secret|bearer-secret|key-secret-exact|session-secret|refresh-secret|credential-secret|key-secret|hidden thought|ciphertext/u,
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test('tool evidence events retain explicit call node and revision correlation', async () => {
  const directory = await temporary();
  try {
    const benchmarkCase = PILOT_CASES[1]!;
    const run = createSchedule({
      studyId: 'tool.correlation',
      cases: [benchmarkCase],
      conditions: [B0],
      seed: 'tool-correlation',
      repetitions: 1,
    })[0]!;
    const events = createEventStore(directory);
    const records = createRecordStore(directory);
    const record = await executeRun(run, benchmarkCase, B0, {
      events,
      records,
      execute: async (context) => {
        await context.callTool(
          'read',
          { path: 'notes/request.md' },
          { callId: 'call-read-001', nodeId: 'g01', revision: 0 },
        );
        return {
          status: 'succeeded',
          outcome: {},
          usage: { inputTokens: 0, outputTokens: 0, costUsd: 0 },
        };
      },
    });
    const calls = (await events.read(run.id, record.attempt))
      .map(({ payload }) => payload)
      .flatMap((payload) => {
        if (
          typeof payload !== 'object' ||
          payload === null ||
          Array.isArray(payload)
        )
          return [];
        const value = payload as Readonly<Record<string, unknown>>;
        return value['type'] === 'tool.call.started' ||
          value['type'] === 'tool.call.finished'
          ? [value]
          : [];
      });
    assert.equal(calls.length, 2);
    assert.ok(
      calls.every(
        (payload) =>
          payload['callId'] === 'call-read-001' &&
          payload['nodeId'] === 'g01' &&
          payload['revision'] === 0,
      ),
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test('failed tool attempts retain correlated lifecycle and sanitized infrastructure status', async () => {
  const directory = await temporary();
  try {
    const benchmarkCase = PILOT_CASES[1]!;
    const run = createSchedule({
      studyId: 'tool.failure',
      cases: [benchmarkCase],
      conditions: [B0],
      seed: 'tool-failure',
      repetitions: 1,
    })[0]!;
    const events = createEventStore(directory);
    const records = createRecordStore(directory);
    const record = await executeRun(run, benchmarkCase, B0, {
      events,
      records,
      execute: async (context) => {
        await context.callTool(
          'read',
          { path: 'missing.txt' },
          { callId: 'call-missing', nodeId: 'g01', revision: 0 },
        );
        throw new Error('unreachable');
      },
    });
    assert.equal(record.status, 'infrastructure');
    assert.equal(record.usage.toolCalls, 1);
    assert.deepEqual(record.infrastructureFailure, {
      stage: 'tool',
      code: 'tool_failed',
      beforeFirstModelCall: true,
    });
    const serialized = JSON.stringify(
      await events.read(run.id, record.attempt),
    );
    assert.match(serialized, /tool\.call\.started/u);
    assert.match(serialized, /tool\.call\.failed/u);
    assert.match(serialized, /call-missing/u);
    assert.doesNotMatch(serialized, /tool input|missing\.txt/u);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test('resume behavior is correct at every crash boundary', async () => {
  const expectations: Readonly<
    Record<CrashBoundary, 'retry-technical' | 'skip-terminal'>
  > = {
    'before-prepare': 'retry-technical',
    'before-first-model': 'retry-technical',
    'after-first-model': 'skip-terminal',
    'after-tool': 'skip-terminal',
    'before-terminal': 'skip-terminal',
    'after-terminal': 'skip-terminal',
  };
  for (const [boundary, expected] of Object.entries(expectations) as [
    CrashBoundary,
    (typeof expectations)[CrashBoundary],
  ][]) {
    const directory = await temporary();
    try {
      const events = createEventStore(directory);
      const records = createRecordStore(directory);
      const run = createSchedule({
        studyId: `crash.${boundary}`,
        cases: PILOT_CASES.slice(0, 1),
        conditions: [M1],
        seed: boundary,
        repetitions: 1,
      })[0]!;
      let crashed = false;
      const invocation = executeRun(run, PILOT_CASES[0]!, M1, {
        events,
        records,
        crash: (current) => {
          if (!crashed && current === boundary) {
            crashed = true;
            throw new HarnessInfrastructureError(
              current === 'after-tool' ? 'tool' : 'model',
              'injected_crash',
            );
          }
        },
        execute: async (context) => {
          await context.startModelCall();
          await context.callTool('read', { path: 'notes/request.md' });
          return {
            status: 'succeeded',
            outcome: { ok: true },
            usage: { inputTokens: 1, outputTokens: 1, costUsd: 0 },
          };
        },
      });
      if (boundary === 'after-terminal') await assert.rejects(invocation);
      else await invocation;
      const attempts = await records.read(run.id);
      assert.equal(attempts.length, 1, boundary);
      assert.equal(resumeAction(attempts), expected, boundary);
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  }
});

test('deterministic evidence evaluator combines trace delivery revision and world state', async () => {
  const directory = await temporary();
  try {
    const benchmarkCase = PILOT_CASES[6]!;
    assert.equal(benchmarkCase.compositionClass, 'A');
    assert.equal(benchmarkCase.gold.requiresRevision, false);
    const run = createSchedule({
      studyId: 'evidence',
      cases: [benchmarkCase],
      conditions: [B0],
      seed: 'evidence',
      repetitions: 1,
    })[0]!;
    const prompts = await loadConditionPrompts();
    const model: BaselineModel = {
      plan: async () => ({
        value: { goals: [] },
        usage: { inputTokens: 0, outputTokens: 0, costUsd: 0 },
      }),
      revise: async () => ({
        value: { id: 'unused', goal: 'unused', doneWhen: ['unused'] },
        usage: { inputTokens: 0, outputTokens: 0, costUsd: 0 },
      }),
      execute: async () => ({
        value: {
          status: 'completed',
          output: 'pilot-07-complete world-v1',
          toolCalls: [],
        },
        usage: { inputTokens: 1, outputTokens: 1, costUsd: 0 },
      }),
    };
    const events = createEventStore(directory);
    const records = createRecordStore(directory);
    const record = await executeRun(run, benchmarkCase, B0, {
      events,
      records,
      execute: (context) => executeBaseline(context, model, prompts),
    });
    const evaluation = evaluateEvidence(
      benchmarkCase,
      record,
      await events.read(run.id, record.attempt),
    );
    assert.equal(evaluation.success, true);
    const scoreEvidence = toScoreEvidence(evaluation, record);
    assert.deepEqual(
      scoreEvidence.assertions.map(({ id, required }) => [id, required]),
      [
        ['terminal', true],
        ['skills', false],
        ['tools', true],
        ['observations', true],
        ['revision', false],
        ['delivery', true],
        ['world', true],
      ],
    );
    assert.deepEqual(scoreEvidence.observations, { actual: [], expected: [] });
    assert.deepEqual(scoreEvidence.actualMenu, TOOL_NAMES);
    assert.deepEqual(scoreEvidence.expectedMenu, TOOL_NAMES);

    const markerOnlyInGoal = {
      ...record,
      outcome: {
        status: 'completed',
        nodes: [{ goal: 'pilot-07-complete world-v1' }],
        delivery: {
          markdown: 'world-v1 without the completion marker',
          parts: [
            {
              id: 'part',
              goal: 'pilot-07-complete',
              markdown: 'world-v1 only',
              artifacts: [],
              observations: [],
            },
          ],
        },
      },
    };
    const falsePositive = evaluateEvidence(
      benchmarkCase,
      markerOnlyInGoal,
      await events.read(run.id, record.attempt),
    );
    assert.equal(falsePositive.deliveryExact, false);
    assert.equal(falsePositive.success, false);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test('condition-internal skill and revision diagnostics do not gate end-to-end success', async () => {
  const directory = await temporary();
  try {
    const benchmarkCase = PILOT_CASES.find(
      (entry) =>
        entry.compositionClass === 'C' && entry.gold.requiresRevision === false,
    )!;
    const marker = benchmarkCase.gold.expectedDelivery.contains.join(' ');
    const run = createSchedule({
      studyId: 'evidence.diagnostics',
      cases: [benchmarkCase],
      conditions: [B0],
      seed: 'diagnostics',
      repetitions: 1,
    })[0]!;
    const events = createEventStore(directory);
    const records = createRecordStore(directory);
    const record = await executeRun(run, benchmarkCase, B0, {
      events,
      records,
      execute: async (context) => {
        await context.emit({
          type: 'baseline.menu',
          goalId: 'g01',
          skillNames: [],
          toolNames: TOOL_NAMES,
        });
        return {
          status: 'succeeded',
          outcome: {
            conditionId: 'B0',
            goals: [{ status: 'completed', output: marker, toolNames: [] }],
          },
          usage: { inputTokens: 0, outputTokens: 0, costUsd: 0 },
        };
      },
    });
    const evaluation = evaluateEvidence(
      benchmarkCase,
      record,
      await events.read(run.id, record.attempt),
    );
    assert.equal(evaluation.skillsExact, false);
    assert.equal(evaluation.revisionExact, true);
    assert.equal(evaluation.success, true);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test('retrieval evidence keeps the configured cutoff and accepts MOSAIC node IDs', async () => {
  const directory = await temporary();
  try {
    const benchmarkCase = PILOT_CASES[0]!;
    const run = createSchedule({
      studyId: 'evidence.retrieval',
      cases: [benchmarkCase],
      conditions: [M1],
      seed: 'retrieval',
      repetitions: 1,
    })[0]!;
    const events = createEventStore(directory);
    const records = createRecordStore(directory);
    const record = await executeRun(run, benchmarkCase, M1, {
      events,
      records,
      execute: async (context) => {
        await context.emit({
          type: 'retrieval.result',
          stage: 'bundle',
          nodeId: 'g01',
          revision: 1,
          k: 5,
          skillNames: ['skill.one', 'skill.two'],
        });
        return {
          status: 'succeeded',
          outcome: {
            delivery: {
              markdown: benchmarkCase.gold.expectedDelivery.contains.join(' '),
              parts: [],
            },
          },
          usage: { inputTokens: 0, outputTokens: 0, costUsd: 0 },
        };
      },
    });
    const evaluation = evaluateEvidence(
      benchmarkCase,
      record,
      await events.read(run.id, record.attempt),
    );
    assert.deepEqual(evaluation.retrievals, [
      { goalId: 'g01', ranked: ['skill.one', 'skill.two'], k: 5 },
    ]);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test('revision evidence requires one correlated observation from the same node and prior revision', async () => {
  const directory = await temporary();
  try {
    const benchmarkCase = PILOT_CASES.find(
      (entry) => entry.id === 'pilot.case.26',
    )!;
    const run = createSchedule({
      studyId: 'evidence.revision',
      cases: [benchmarkCase],
      conditions: [B3],
      seed: 'revision',
      repetitions: 1,
    })[0]!;
    const events = createEventStore(directory);
    const records = createRecordStore(directory);
    const record = await executeRun(run, benchmarkCase, B3, {
      events,
      records,
      execute: async () => ({
        status: 'succeeded',
        outcome: {
          conditionId: 'B3',
          goals: [
            {
              status: 'completed',
              output: benchmarkCase.gold.expectedDelivery.contains.join(' '),
              toolNames: ['read'],
            },
          ],
        },
        usage: { inputTokens: 0, outputTokens: 0, costUsd: 0 },
      }),
    });
    const expected = benchmarkCase.gold.expectedState.toolEvidence[0]!;
    await events.append(run.id, record.attempt, {
      type: 'tool.call.started',
      name: 'read',
      callId: 'call-other',
      nodeId: 'other',
      revision: 0,
    });
    await events.append(run.id, record.attempt, {
      type: 'tool.call.started',
      name: 'read',
      callId: 'call-target',
      nodeId: 'target',
      revision: 0,
    });
    await events.append(run.id, record.attempt, {
      type: 'tool.call.finished',
      name: 'read',
      callId: 'call-orphan',
      nodeId: 'target',
      revision: 0,
      evidenceHash: expected.evidenceHash,
    });
    await events.append(run.id, record.attempt, {
      type: 'tool.call.finished',
      name: 'read',
      callId: 'call-target',
      nodeId: 'target',
      revision: 0,
      evidenceHash: expected.evidenceHash,
    });
    await events.append(run.id, record.attempt, {
      type: 'baseline.revision',
      goalId: 'target',
      revision: 1,
    });
    const stored = await events.read(run.id, record.attempt);
    const matching = evaluateEvidence(benchmarkCase, record, stored);
    assert.equal(matching.observationExact, true);
    assert.equal(matching.revisionExact, true);

    const wrongNode = stored.map((event) => {
      if (
        typeof event.payload !== 'object' ||
        event.payload === null ||
        Array.isArray(event.payload)
      )
        return event;
      const payload = event.payload as Readonly<
        Record<string, HarnessJsonValue>
      >;
      return payload['type'] === 'baseline.revision'
        ? { ...event, payload: { ...payload, goalId: 'other' } }
        : event;
    });
    assert.equal(
      evaluateEvidence(benchmarkCase, record, wrongNode).revisionExact,
      false,
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test('state oracle executes frozen tool evidence and reaches the exact World hash', async () => {
  const directory = await temporary();
  try {
    const benchmarkCase = PILOT_CASES.find(
      (entry) => entry.composition.requiresExternalEffect,
    )!;
    const events = createEventStore(directory);
    const records = createRecordStore(directory);
    const parentRun = createSchedule({
      studyId: 'oracle.parent',
      cases: [benchmarkCase],
      conditions: [B0],
      seed: 'parent',
      repetitions: 1,
    })[0]!;
    const parent = await executeRun(parentRun, benchmarkCase, B0, {
      events,
      records,
      execute: async () => ({
        status: 'failed',
        outcome: { failureCode: 'execution_failed' },
        usage: { inputTokens: 0, outputTokens: 0, costUsd: 0 },
      }),
    });
    const stateOracle = conditionById('O_STATE');
    const oracleRun = createSchedule({
      studyId: 'oracle.state',
      cases: [benchmarkCase],
      conditions: [stateOracle],
      seed: 'oracle',
      repetitions: 1,
    })[0]!;
    const oracle = await executeRun(
      oracleRun,
      benchmarkCase,
      stateOracle,
      {
        events,
        records,
        execute: async (context) => {
          const hook = evaluationHooks(
            stateOracle,
            benchmarkCase,
            oracleRun.seed,
            {},
            undefined,
            context.startModelCall,
            context.emit,
            context,
          ).execution!;
          const result = await hook(
            {
              request: benchmarkCase.request,
              graph: { revision: 1, nodes: [] },
              node: {
                id: 'g01',
                goal: benchmarkCase.request,
                doneWhen: benchmarkCase.gold.criteria.map(
                  ({ description }) => description,
                ),
                dependsOn: [],
                deliver: true,
              },
              skills: [],
              tools: [],
            } as never,
            async () => {
              throw new Error('state oracle must not call the model executor');
            },
          );
          await hook(
            {
              request: benchmarkCase.request,
              graph: { revision: 1, nodes: [] },
              node: {
                id: 'g02',
                goal: 'Second oracle node',
                doneWhen: ['No repeated effect.'],
                dependsOn: ['g01'],
                deliver: false,
              },
              skills: [],
              tools: [],
            } as never,
            async () => {
              throw new Error('state oracle must not call the model executor');
            },
          );
          return {
            status: 'succeeded',
            outcome: {
              status: 'completed',
              delivery: {
                markdown: result.decision.result?.markdown ?? '',
                parts: [],
              },
              nodes: [],
            },
            usage: { inputTokens: 0, outputTokens: 0, costUsd: 0 },
          };
        },
      },
      parent,
    );
    const evaluation = evaluateEvidence(
      benchmarkCase,
      oracle,
      await events.read(oracleRun.id, oracle.attempt),
    );
    assert.equal(evaluation.worldExact, true);
    assert.equal(evaluation.observationExact, true);
    assert.deepEqual(evaluation.actualEvidence, evaluation.expectedEvidence);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test('incorrect tool input cannot satisfy frozen evidence despite the right tool name', async () => {
  const directory = await temporary();
  try {
    const benchmarkCase = PILOT_CASES[1]!;
    assert.equal(benchmarkCase.compositionClass, 'B');
    const run = createSchedule({
      studyId: 'evidence.input',
      cases: [benchmarkCase],
      conditions: [B0],
      seed: 'wrong-input',
      repetitions: 1,
    })[0]!;
    const events = createEventStore(directory);
    const records = createRecordStore(directory);
    const record = await executeRun(run, benchmarkCase, B0, {
      events,
      records,
      execute: async (context) => {
        await context.callTool('read', { path: 'data/settings.json' });
        return {
          status: 'succeeded',
          outcome: {
            conditionId: 'B0',
            goals: [
              {
                status: 'completed',
                output: 'pilot-02-complete world-v1',
                toolNames: ['read'],
              },
            ],
          },
          usage: { inputTokens: 0, outputTokens: 0, costUsd: 0 },
        };
      },
    });
    const evaluation = evaluateEvidence(
      benchmarkCase,
      record,
      await events.read(run.id, record.attempt),
    );
    assert.equal(evaluation.toolsExact, true);
    assert.equal(evaluation.observationExact, false);
    assert.equal(evaluation.success, false);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test('model-call budget cap becomes an analyzable domain failure', async () => {
  const directory = await temporary();
  try {
    const scheduled = createSchedule({
      studyId: 'budget',
      cases: PILOT_CASES.slice(0, 1),
      conditions: [M1],
      seed: 'budget',
      repetitions: 1,
    })[0]!;
    const run = { ...scheduled, modelCallBudget: 1 };
    const events = createEventStore(directory);
    const records = createRecordStore(directory);
    const record = await executeRun(run, PILOT_CASES[0]!, M1, {
      events,
      records,
      usage: () => ({
        inputTokens: 10,
        outputTokens: 5,
        cachedInputTokens: 2,
        costUsd: 0.2,
      }),
      execute: async (context) => {
        await context.startModelCall();
        await context.startModelCall();
        return {
          status: 'succeeded',
          outcome: { unreachable: true },
          usage: { inputTokens: 0, outputTokens: 0, costUsd: 0 },
        };
      },
    });
    assert.equal(record.status, 'failed');
    assert.equal(record.infrastructureFailure, null);
    assert.equal(record.usage.modelCalls, 1);
    assert.equal(record.usage.inputTokens, 10);
    assert.equal(record.usage.outputTokens, 5);
    assert.equal(record.usage.cachedInputTokens, 2);
    assert.equal(record.usage.costUsd, 0.2);
    assert.deepEqual(record.outcome, {
      failureCode: 'model_call_budget',
      modelCalls: 1,
    });
    assert.equal(resumeAction([record]), 'skip-terminal');
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test('post-model provider failure preserves partial usage in its terminal record', async () => {
  const directory = await temporary();
  try {
    const run = createSchedule({
      studyId: 'partial.usage',
      cases: PILOT_CASES.slice(0, 1),
      conditions: [M1],
      seed: 'partial',
      repetitions: 1,
    })[0]!;
    const events = createEventStore(directory);
    const records = createRecordStore(directory);
    const record = await executeRun(run, PILOT_CASES[0]!, M1, {
      events,
      records,
      usage: () => ({
        inputTokens: 21,
        outputTokens: 8,
        reasoningTokens: 3,
        costUsd: 0.42,
      }),
      execute: async (context) => {
        await context.startModelCall();
        throw new HarnessInfrastructureError('model', 'provider_failed');
      },
    });
    assert.equal(record.status, 'infrastructure');
    assert.deepEqual(record.infrastructureFailure, {
      stage: 'model',
      code: 'provider_failed',
      beforeFirstModelCall: false,
    });
    assert.deepEqual(record.usage, {
      inputTokens: 21,
      outputTokens: 8,
      reasoningTokens: 3,
      modelCalls: 1,
      toolCalls: 0,
      costUsd: 0.42,
    });
    assert.equal(resumeAction([record]), 'skip-terminal');
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test('event-store and observer failures keep their allowlisted infrastructure provenance', async () => {
  const directory = await temporary();
  try {
    const backingEvents = createEventStore(directory);
    const records = createRecordStore(directory);
    const run = createSchedule({
      studyId: 'provenance.store',
      cases: PILOT_CASES.slice(0, 1),
      conditions: [M1],
      seed: 'store',
      repetitions: 1,
    })[0]!;
    let failAppend = true;
    const storeRecord = await executeRun(run, PILOT_CASES[0]!, M1, {
      events: {
        ...backingEvents,
        append: async (runId, attempt, payload) => {
          if (failAppend) {
            failAppend = false;
            throw new Error('raw filesystem detail');
          }
          return backingEvents.append(runId, attempt, payload);
        },
      },
      records,
      execute: async () => ({
        status: 'succeeded',
        outcome: {},
        usage: { inputTokens: 0, outputTokens: 0, costUsd: 0 },
      }),
    });
    assert.deepEqual(storeRecord.infrastructureFailure, {
      stage: 'store',
      code: 'event_append_failed',
      beforeFirstModelCall: true,
    });

    const observerRun = createSchedule({
      studyId: 'provenance.observer',
      cases: PILOT_CASES.slice(0, 1),
      conditions: [M1],
      seed: 'observer',
      repetitions: 1,
    })[0]!;
    const retrievers = createTestLexicalRetrievers();
    const mosaic = {
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
        _options: Parameters<NonNullable<MosaicDependencies['factory']>>[0],
      ) => ({
        prompt: async (
          _input: string,
          options?: { readonly observer?: (event: never) => Promise<void> },
        ) => {
          const event: Record<string, unknown> = {
            schemaVersion: 1,
            runId: 'mosaic-observer',
            sequence: 1,
            type: 'run.started',
            stage: 'run',
          };
          event['prompt'] = event;
          await options?.observer?.(event as never);
          throw new Error('unreachable after observer failure');
        },
      })) as NonNullable<MosaicDependencies['factory']>,
    } satisfies MosaicDependencies;
    const observerRecord = await executeRun(observerRun, PILOT_CASES[0]!, M1, {
      events: backingEvents,
      records,
      execute: (context) => executeMosaic(context, mosaic),
    });
    assert.deepEqual(observerRecord.infrastructureFailure, {
      stage: 'observer',
      code: 'observer_failed',
      beforeFirstModelCall: true,
    });
    assert.doesNotMatch(
      JSON.stringify(await backingEvents.read(run.id, storeRecord.attempt)),
      /raw filesystem detail/u,
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test('technical retry starts an independent trace attempt and record sequence', async () => {
  const directory = await temporary();
  try {
    const events = createEventStore(directory);
    const records = createRecordStore(directory);
    const run = createSchedule({
      studyId: 'attempt.isolation',
      cases: PILOT_CASES.slice(0, 1),
      conditions: [M1],
      seed: 'attempt',
      repetitions: 1,
    })[0]!;
    const first = await executeRun(run, PILOT_CASES[0]!, M1, {
      events,
      records,
      execute: async () => {
        throw new HarnessInfrastructureError('prepare', 'injected_crash');
      },
    });
    const second = await executeRun(run, PILOT_CASES[0]!, M1, {
      events,
      records,
      execute: async () => ({
        status: 'succeeded',
        outcome: { delivery: { markdown: 'complete', parts: [] } },
        usage: { inputTokens: 0, outputTokens: 0, costUsd: 0 },
      }),
    });
    assert.deepEqual([first.attempt, second.attempt], [1, 2]);
    const firstEvents = await events.read(run.id, first.attempt);
    const secondEvents = await events.read(run.id, second.attempt);
    assert.equal(firstEvents[0]?.sequence, 1);
    assert.equal(secondEvents[0]?.sequence, 1);
    assert.ok(firstEvents.every((event) => event.attempt === 1));
    assert.ok(secondEvents.every((event) => event.attempt === 2));
    assert.notEqual(first.trace.derivedHash, second.trace.derivedHash);
    assert.deepEqual(
      (await records.read(run.id)).map((record) => record.attempt),
      [1, 2],
    );
    await assert.rejects(
      records.append({ ...second, attempt: 4 }),
      /next immutable sequence/u,
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
