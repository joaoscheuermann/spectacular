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
  const stderr = output();
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
      stderr,
      resume: async (options) => {
        received = options;
        (
          options as ResumeOptions & {
            readonly progress?: (stage: string) => void;
          }
        ).progress?.('running Direct');
        return { directory: options.campaignDir, arms: [] };
      },
    },
  );

  assert.equal(code, 0);
  assert.equal(received?.benchmark, 'skillsbench');
  assert.equal(received?.campaignDir, '/bench/results/pilot');
  assert.equal(received?.rootDir, '/bench');
  assert.equal(received?.yesPaidRun, true);
  assert.equal(
    JSON.parse(stdout.lines[0] ?? '{}').directory,
    '/bench/results/pilot',
  );
  assert.deepEqual(stderr.lines, ['[mosaic-benchmark] running Direct\n']);
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
