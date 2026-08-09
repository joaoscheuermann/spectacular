import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { spawn } from 'node:child_process';
import test from 'node:test';

import { M1 } from '../src/conditions/index.js';
import {
  createEventStore,
  createRecordStore,
  executeRun,
  recoverInterruptedRun,
  type CrashBoundary,
} from '../src/runtime/index.js';
import {
  PILOT_CASES,
  createSchedule,
  resumeAction,
} from '../src/study/index.js';

const temporary = (): Promise<string> =>
  mkdtemp(resolve(tmpdir(), 'mosaic-abrupt-'));

const fixture = resolve('benchmarks/mosaic/tests/fixtures/abrupt-runner.mjs');

const killAt = async (root: string, boundary: CrashBoundary): Promise<void> =>
  new Promise((resolveChild, reject) => {
    const child = spawn(process.execPath, [fixture, root, boundary], {
      stdio: ['ignore', 'ignore', 'pipe'],
    });
    const diagnostics: Buffer[] = [];
    child.stderr.on('data', (chunk: Buffer) => diagnostics.push(chunk));
    child.once('error', reject);
    child.once('exit', (code, signal) => {
      if (code === null && signal === 'SIGKILL') resolveChild();
      else {
        const detail = Buffer.concat(diagnostics).toString('utf8').trim();
        reject(
          new Error(
            `child did not die by SIGKILL: ${code}/${signal}${detail.length === 0 ? '' : `: ${detail}`}`,
          ),
        );
      }
    });
  });

const scheduled = (boundary: CrashBoundary) => {
  const benchmarkCase = PILOT_CASES[1]!;
  const run = createSchedule({
    studyId: `abrupt.${boundary}`,
    cases: [benchmarkCase],
    conditions: [M1],
    seed: boundary,
    repetitions: 1,
  })[0]!;
  return { benchmarkCase, run };
};

test('SIGKILL before preparation is recovered and permits one technical retry', async () => {
  const directory = await temporary();
  try {
    const { benchmarkCase, run } = scheduled('before-prepare');
    await killAt(directory, 'before-prepare');
    const events = createEventStore(directory);
    const records = createRecordStore(directory);
    const recovered = await recoverInterruptedRun(run, { events, records });
    assert.equal(recovered?.attempt, 1);
    assert.equal(recovered?.infrastructureFailure?.code, 'interrupted');
    assert.equal(recovered?.firstModelCallStarted, false);
    assert.equal(resumeAction(await records.read(run.id)), 'retry-technical');

    const retry = await executeRun(run, benchmarkCase, M1, {
      events,
      records,
      execute: async () => ({
        status: 'succeeded',
        outcome: {},
        usage: { inputTokens: 0, outputTokens: 0, costUsd: 0 },
      }),
    });
    assert.equal(retry.attempt, 2);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test('SIGKILL after paid activity is terminal at every persistence boundary', async () => {
  const boundaries: readonly CrashBoundary[] = [
    'after-first-model',
    'after-tool',
    'before-terminal',
    'after-trace',
    'before-record',
  ];
  for (const boundary of boundaries) {
    const directory = await temporary();
    try {
      const { run } = scheduled(boundary);
      await killAt(directory, boundary);
      const events = createEventStore(directory);
      const records = createRecordStore(directory);
      const recovered = await recoverInterruptedRun(run, { events, records });
      assert.equal(recovered?.attempt, 1, boundary);
      assert.equal(recovered?.firstModelCallStarted, true, boundary);
      assert.equal(
        resumeAction(await records.read(run.id)),
        'skip-terminal',
        boundary,
      );
      assert.equal((await records.pending(run.id)).length, 0, boundary);
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  }
});

test('a terminal record survives SIGKILL after append without reconciliation', async () => {
  const directory = await temporary();
  try {
    const { run } = scheduled('after-terminal');
    await killAt(directory, 'after-terminal');
    const events = createEventStore(directory);
    const records = createRecordStore(directory);
    assert.equal(
      await recoverInterruptedRun(run, { events, records }),
      undefined,
    );
    assert.equal((await records.read(run.id)).length, 1);
    assert.equal((await records.pending(run.id)).length, 0);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
