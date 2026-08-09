import assert from 'node:assert/strict';
import test from 'node:test';

import { B0, B1, B2, B3, M1 } from '../src/conditions/index.js';
import { artifactHash, canonicalJson } from '../src/core/index.js';
import type { JsonValue } from '../src/core/json.js';
import type { Condition, ExecutionRecord } from '../src/schemas/index.js';
import type { StoredEvent } from '../src/runtime/index.js';
import {
  PILOT_CASES,
  createSchedule,
  evaluateEvidence,
} from '../src/study/index.js';

const hash = artifactHash([]);

const recordFor = (
  benchmarkCase: (typeof PILOT_CASES)[number],
  delivery: string,
  condition: Condition = B0,
): ExecutionRecord => {
  const run = createSchedule({
    studyId: 'semantic-evidence',
    cases: [benchmarkCase],
    conditions: [condition],
    seed: benchmarkCase.id,
    repetitions: 1,
  })[0]!;
  return {
    schemaVersion: 1,
    attempt: 1,
    run,
    status: 'succeeded',
    startedAt: '2026-08-09T00:00:00.000Z',
    finishedAt: '2026-08-09T00:00:01.000Z',
    durationMs: 1_000,
    firstModelCallStarted: true,
    trace: {
      rootHash: hash,
      derivedHash: hash,
      eventCount: benchmarkCase.gold.expectedState.toolEvidence.length * 2,
      relativePath: 'traces/semantic-evidence.json',
    },
    outcome: { goals: [{ status: 'completed', output: delivery }] },
    worldHash: benchmarkCase.gold.expectedState.worldHash,
    usage: {
      inputTokens: 1,
      outputTokens: 1,
      modelCalls: 1,
      toolCalls: benchmarkCase.gold.expectedState.toolEvidence.length,
      costUsd: 0,
    },
    infrastructureFailure: null,
  };
};

const eventsFor = (record: ExecutionRecord): readonly StoredEvent[] =>
  PILOT_CASES.find(
    ({ id }) => id === record.run.caseId,
  )!.gold.expectedState.toolEvidence.flatMap((evidence, index) => {
    const callId = `call-${index + 1}`;
    const values = [
      {
        type: 'tool.call.started',
        name: evidence.name,
        callId,
      },
      {
        type: 'tool.call.finished',
        name: evidence.name,
        callId,
        evidenceHash: evidence.evidenceHash,
      },
    ] as const;
    return values.map((payload, offset) => ({
      schemaVersion: 1 as const,
      runId: record.run.id,
      attempt: record.attempt,
      sequence: index * 2 + offset + 1,
      previousHash: null,
      payloadHash: hash,
      payload,
      eventHash: hash,
    }));
  });

test('accepts the canonical semantic delivery for every composition class', () => {
  for (const compositionClass of ['A', 'B', 'C', 'D', 'E', 'F'] as const) {
    const benchmarkCase = PILOT_CASES.find(
      (entry) =>
        entry.compositionClass === compositionClass &&
        !entry.gold.requiresRevision,
    )!;
    const record = recordFor(
      benchmarkCase,
      `\`\`\`json\n${canonicalJson(benchmarkCase.gold.expectedDelivery.document)}\n\`\`\``,
    );

    const evaluation = evaluateEvidence(
      benchmarkCase,
      record,
      eventsFor(record),
    );

    assert.equal(evaluation.deliveryExact, true, compositionClass);
    assert.equal(evaluation.criteriaExact, true, compositionClass);
    assert.equal(evaluation.success, true, compositionClass);
  }
});

test('uses identical task evidence for B0 through B3 and M1', () => {
  const benchmarkCase = PILOT_CASES.find(
    (entry) => entry.compositionClass === 'A',
  )!;
  const delivery = canonicalJson(benchmarkCase.gold.expectedDelivery.document);

  for (const condition of [B0, B1, B2, B3, M1]) {
    const record = recordFor(benchmarkCase, delivery, condition);
    const evaluation = evaluateEvidence(benchmarkCase, record, []);
    assert.equal(evaluation.deliveryExact, true, condition.id);
    assert.equal(evaluation.criteriaExact, true, condition.id);
    assert.equal(evaluation.success, true, condition.id);
  }
});

test('rejects marker echoes omitted fields and incorrect canonical facts', () => {
  const benchmarkCase = PILOT_CASES.find(
    (entry) => entry.compositionClass === 'A',
  )!;
  const marker = recordFor(benchmarkCase, 'pilot-01-complete world-v1');
  assert.equal(
    evaluateEvidence(benchmarkCase, marker, []).deliveryExact,
    false,
  );

  const expected = benchmarkCase.gold.expectedDelivery.document;
  const omitted = { ...expected };
  delete omitted['answer'];
  const omission = recordFor(benchmarkCase, canonicalJson(omitted));
  const omissionResult = evaluateEvidence(benchmarkCase, omission, []);
  assert.equal(omissionResult.deliveryExact, false);
  assert.ok(omissionResult.criteria.some(({ passed }) => !passed));

  const incorrect = recordFor(
    benchmarkCase,
    canonicalJson({ ...expected, answer: { incorrect: true } }),
  );
  assert.equal(
    evaluateEvidence(benchmarkCase, incorrect, []).deliveryExact,
    false,
  );

  const contradictory = recordFor(
    benchmarkCase,
    `\`\`\`json\n${canonicalJson(expected)}\n\`\`\`\n\n\`\`\`json\n${canonicalJson({ ...expected, answer: { incorrect: true } })}\n\`\`\``,
  );
  assert.equal(
    evaluateEvidence(benchmarkCase, contradictory, []).deliveryExact,
    false,
  );
});

test('rejects incorrect effect evidence and final world state', () => {
  const benchmarkCase = PILOT_CASES.find(
    (entry) =>
      entry.gold.requiredTools.includes('message_send') ||
      entry.gold.requiredTools.includes('artifact_publish') ||
      entry.gold.requiredTools.includes('write'),
  )!;
  const record = recordFor(
    benchmarkCase,
    canonicalJson(benchmarkCase.gold.expectedDelivery.document),
  );
  const events = eventsFor(record);
  const incorrectEffect = events.map((event) => {
    const payload = event.payload as Readonly<Record<string, JsonValue>>;
    return payload['type'] === 'tool.call.finished'
      ? {
          ...event,
          payload: { ...payload, evidenceHash: artifactHash('wrong effect') },
        }
      : event;
  });

  const effectResult = evaluateEvidence(benchmarkCase, record, incorrectEffect);
  assert.equal(effectResult.observationExact, false);
  assert.equal(effectResult.criteriaExact, false);
  assert.equal(effectResult.success, false);

  const stateResult = evaluateEvidence(
    benchmarkCase,
    { ...record, worldHash: hash },
    events,
  );
  assert.equal(stateResult.worldExact, false);
  assert.equal(stateResult.success, false);
});

test('pilot requests contain no completion-marker password', () => {
  assert.ok(
    PILOT_CASES.every((entry) => !/pilot-\d+-complete/u.test(entry.request)),
  );
});
