import assert from 'node:assert/strict';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import { runCli } from '../src/cli.js';
import type { CampaignOptions } from '../src/campaign.js';

const output = (): {
  readonly lines: string[];
  write(value: string): boolean;
} => {
  const lines: string[] = [];
  return { lines, write: (value) => (lines.push(value), true) };
};

test('dispatches the ACP mode without writing protocol output itself', async () => {
  const stdout = output();
  let mode = '';
  const code = await runCli(['serve', 'mosaic'], {
    stdout,
    serveAcp: async (options) => {
      mode = options.mode;
    },
  });

  assert.equal(code, 0);
  assert.equal(mode, 'mosaic');
  assert.deepEqual(stdout.lines, []);
});

test('keeps paid campaign execution behind the explicit flag', async () => {
  const stdout = output();
  let received: CampaignOptions | undefined;
  const code = await runCli(
    ['campaign', 'skillsbench', 'smoke', '--yes-paid-run', '--root', '/bench'],
    {
      stdout,
      campaign: async (options) => {
        received = options;
        return { directory: '/result', arms: [] };
      },
    },
  );

  assert.equal(code, 0);
  assert.deepEqual(received, {
    benchmark: 'skillsbench',
    action: 'smoke',
    rootDir: '/bench',
    yesPaidRun: true,
  });
  assert.equal(JSON.parse(stdout.lines[0] ?? '{}').directory, '/result');
});

test('dispatches the fixed SkillsBench pilot action', async () => {
  let received: CampaignOptions | undefined;
  const code = await runCli(
    ['campaign', 'skillsbench', 'pilot', '--yes-paid-run', '--root', '/bench'],
    {
      stdout: output(),
      campaign: async (options) => {
        received = options;
        return { directory: '/result', arms: [] };
      },
    },
  );

  assert.equal(code, 0);
  assert.equal(received?.action, 'pilot');
});

test('returns the comparison decision exit code', async () => {
  const stdout = output();
  const code = await runCli(
    ['compare', '--direct', '/direct', '--mosaic', '/mosaic'],
    {
      stdout,
      compare: async () => ({
        benchmark: 'skillsbench',
        valid: true,
        reasons: [],
        direct: {
          score: 0.5,
          reward: 0.5,
          costUsd: 2,
          costPerRewardUsd: 4,
          totalTokens: 10,
          tasks: 1,
        },
        mosaic: {
          score: 1,
          reward: 1,
          costUsd: 1,
          costPerRewardUsd: 1,
          totalTokens: 9,
          tasks: 1,
        },
        paired: {
          scoreDelta: 0.5,
          qualityWin: true,
          mosaicWins: ['task'],
          regressions: [],
          ties: [],
        },
        paretoWin: true,
        exitCode: 0,
      }),
    },
  );

  assert.equal(code, 0);
  assert.equal(JSON.parse(stdout.lines[0] ?? '{}').paretoWin, true);
});

test('forwards the SkillsBench evidence gate for Terminal-Bench', async () => {
  let received: CampaignOptions | undefined;
  const code = await runCli(
    [
      'campaign',
      'terminalbench',
      'smoke',
      '--yes-paid-run',
      '--skillsbench-report',
      '/results/skillsbench.json',
      '--root',
      '/bench',
    ],
    {
      stdout: output(),
      campaign: async (options) => {
        received = options;
        return { directory: '/result', arms: [] };
      },
    },
  );

  assert.equal(code, 0);
  assert.equal(received?.skillsbenchReport, '/results/skillsbench.json');
});

test('creates the release checksum beside the generated bundle', async () => {
  const root = await mkdtemp(join(tmpdir(), 'mosaic-cli-'));
  try {
    await mkdir(join(root, 'dist'));
    await writeFile(join(root, 'project.json'), '{}');
    await writeFile(join(root, 'dist', 'mosaic-bench-acp.mjs'), 'asset');
    const stdout = output();

    assert.equal(await runCli(['release'], { cwd: root, stdout }), 0);
    assert.equal(JSON.parse(stdout.lines[0] ?? '{}').version, '0.1.12');
    const checksum = await readFile(
      join(root, 'dist', 'mosaic-bench-acp.mjs.sha256'),
      'utf8',
    );
    assert.match(checksum, /^[a-f0-9]{64}  mosaic-bench-acp\.mjs\n$/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('rejects unknown arguments without writing to stdout', async () => {
  const stdout = output();
  const stderr = output();
  const code = await runCli(['serve', 'direct', '--unknown'], {
    stdout,
    stderr,
  });

  assert.equal(code, 2);
  assert.deepEqual(stdout.lines, []);
  assert.match(stderr.lines[0] ?? '', /Unknown argument/);
});
