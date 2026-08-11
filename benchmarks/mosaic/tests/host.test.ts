import assert from 'node:assert/strict';
import test from 'node:test';

import { runHost } from '../src/host.js';
import type { ResumeOptions } from '../src/resume.js';

const output = (): {
  readonly lines: string[];
  write(value: string): boolean;
} => {
  const lines: string[] = [];
  return { lines, write: (value) => (lines.push(value), true) };
};

test('dispatches campaign resume through the host command', async () => {
  const stdout = output();
  let received: ResumeOptions | undefined;
  const code = await runHost(
    [
      'campaign',
      'skillsbench',
      'resume',
      '--campaign',
      '/bench/results/pilot',
      '--yes-paid-run',
      '--root',
      '/bench',
    ],
    {
      stdout,
      resume: async (options) => {
        received = options;
        return { directory: options.campaignDir, arms: [] };
      },
    },
  );

  assert.equal(code, 0);
  assert.deepEqual(received, {
    benchmark: 'skillsbench',
    campaignDir: '/bench/results/pilot',
    rootDir: '/bench',
    yesPaidRun: true,
  });
  assert.equal(
    JSON.parse(stdout.lines[0] ?? '{}').directory,
    '/bench/results/pilot',
  );
});

test('delegates every non-resume command to the released CLI', async () => {
  const stdout = output();
  let delegated: readonly string[] = [];
  const code = await runHost(['help'], {
    stdout,
    delegate: async (args) => {
      delegated = args;
      return 0;
    },
  });

  assert.equal(code, 0);
  assert.deepEqual(delegated, ['help']);
  assert.match(stdout.lines[0] ?? '', /campaign skillsbench resume/);
});
