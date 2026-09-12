import assert from 'node:assert/strict';
import { resolve } from 'node:path';
import test from 'node:test';

import { type DockerCommand,runDocker } from '../src/docker.js';

const workspace = process.cwd();
const root = resolve(workspace, 'benchmarks', 'mosaic');

const output = (): {
  readonly lines: string[];
  write(value: string): boolean;
} => {
  const lines: string[] = [];

  return { lines, write: (value) => (lines.push(value), true) };
};

test('builds the coordinator image before running a campaign', async () => {
  const commands: DockerCommand[] = [];

  const code = await runDocker(['campaign', 'skillsbench', 'check'], {
    cwd: workspace,
    environment: { OPENROUTER_API_KEY: 'secret-value' },
    execute: async (command) => (commands.push(command), 0),
  });

  assert.equal(code, 0);

  assert.equal(commands.length, 2);

  assert.deepEqual(commands[0], {
    file: 'docker',
    args: ['build', '--tag', 'mosaic-benchmark:local', '.'],
    cwd: root,
    environment: { OPENROUTER_API_KEY: 'secret-value' },
  });

  const run = commands[1];

  assert.equal(run?.file, 'docker');

  assert.equal(run?.cwd, root);

  assert.ok(run?.args.includes('--privileged'));

  assert.ok(
    run?.args.includes(
      `type=bind,source=${resolve(root, 'results')},target=/benchmark/results`,
    ),
  );

  assert.ok(
    run?.args.includes(
      'type=volume,source=mosaic-benchmark-docker,target=/var/lib/docker',
    ),
  );

  assert.ok(
    run?.args.includes(
      'type=volume,source=mosaic-benchmark-uv,target=/root/.cache/uv',
    ),
  );

  assert.ok(!run?.args.some((argument) => argument.includes('docker.sock')));

  assert.deepEqual(run?.args.slice(-4), [
    'mosaic-benchmark:local',
    'campaign',
    'skillsbench',
    'check',
  ]);

  assert.ok(run?.args.includes('OPENROUTER_API_KEY'));

  assert.ok(!run?.args.some((argument) => argument.includes('secret-value')));
});

test('translates campaign result paths into the mounted directory', async () => {
  const commands: DockerCommand[] = [];
  const campaign = resolve(root, 'results', 'pilot-campaign');
  const report = resolve(root, 'results', 'full-campaign', 'compare.json');

  const code = await runDocker(
    [
      'campaign',
      'skillsbench',
      'resume',
      '--campaign',
      campaign,
      '--skillsbench-report',
      report,
      '--root',
      root,
      '--yes-paid-run',
    ],
    {
      cwd: workspace,
      environment: {},
      execute: async (command) => (commands.push(command), 0),
    },
  );

  assert.equal(code, 0);

  assert.deepEqual(commands[1]?.args.slice(-10), [
    'campaign',
    'skillsbench',
    'resume',
    '--campaign',
    '/benchmark/results/pilot-campaign',
    '--skillsbench-report',
    '/benchmark/results/full-campaign/compare.json',
    '--root',
    '/benchmark',
    '--yes-paid-run',
  ]);
});

test('rejects commands outside the campaign boundary', async () => {
  const stderr = output();
  let executions = 0;

  const code = await runDocker(['compare'], {
    cwd: workspace,
    stderr,
    execute: async () => (executions += 1),
  });

  assert.equal(code, 2);

  assert.equal(executions, 0);

  assert.match(stderr.lines.join(''), /only supports campaign commands/i);
});

test('rejects campaign inputs outside the mounted results directory', async () => {
  const stderr = output();
  let executions = 0;

  const code = await runDocker(
    [
      'campaign',
      'skillsbench',
      'resume',
      '--campaign',
      resolve(workspace, 'outside-results'),
    ],
    {
      cwd: workspace,
      stderr,
      execute: async () => (executions += 1),
    },
  );

  assert.equal(code, 2);

  assert.equal(executions, 0);

  assert.match(stderr.lines.join(''), /must be inside.*results/i);
});

test('stops when the coordinator image cannot be built', async () => {
  const commands: DockerCommand[] = [];

  const code = await runDocker(['campaign', 'skillsbench', 'check'], {
    cwd: workspace,
    execute: async (command) => (commands.push(command), 17),
  });

  assert.equal(code, 17);

  assert.equal(commands.length, 1);
});
