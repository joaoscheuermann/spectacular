import { resolve } from 'node:path';

import { conditions, runtime, study } from '../../dist/src/index.js';

const root = resolve(process.argv[2]);
const boundary = process.argv[3];
const benchmarkCase = study.PILOT_CASES[1];
const run = study.createSchedule({
  studyId: `abrupt.${boundary}`,
  cases: [benchmarkCase],
  conditions: [conditions.M1],
  seed: boundary,
  repetitions: 1,
})[0];
const events = runtime.createEventStore(root);
const records = runtime.createRecordStore(root);

await runtime.executeRun(run, benchmarkCase, conditions.M1, {
  events,
  records,
  crash: async (current) => {
    if (current === boundary) process.kill(process.pid, 'SIGKILL');
  },
  execute: async (context) => {
    await context.startModelCall();
    await context.callTool('read', { path: 'notes/request.md' });
    return {
      status: 'succeeded',
      outcome: { delivery: { markdown: 'complete', parts: [] } },
      usage: { inputTokens: 1, outputTokens: 1, costUsd: 0.01 },
    };
  },
});
