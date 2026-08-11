import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import {
  mkdtemp,
  mkdir,
  readFile,
  rm,
  stat,
  writeFile,
} from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { basename, join } from 'node:path';
import test from 'node:test';

import {
  campaign as executeCampaign,
  type CampaignOptions,
  type Command,
  type CommandResult,
} from '../src/campaign.js';

const skillsCommit = 'b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af';
const terminalCommit = '2fd12b88aafdd04a52c298e3940bcb189f9766d6';
const providerEnvironment = { OPENROUTER_API_KEY: 'test-key' };
const pilotTasks = [
  'data-to-d3',
  'earthquake-phase-association',
  'edit-pdf',
  'jax-computing-basics',
  'organize-messy-files',
  'pptx-reference-formatting',
  'sec-financial-report',
  'spring-boot-jakarta-migration',
  'travel-planning',
  'xlsx-recover-data',
] as const;
const fullTaskNames = Array.from(
  { length: 87 },
  (_, index) => `task-${index + 1}`,
);
const fullMetrics = (score: number, costUsd: number, totalTokens: number) => {
  const reward = score * 87;
  return {
    score,
    reward,
    costUsd,
    costPerRewardUsd: costUsd / reward,
    totalTokens,
    tasks: 87,
  };
};
const fullPaired = (qualityWin: boolean) => ({
  scoreDelta: qualityWin ? 0.3 : -0.3,
  qualityWin,
  mosaicWins: qualityWin ? fullTaskNames : [],
  regressions: qualityWin ? [] : fullTaskNames,
  ties: [],
});
const campaign = (options: CampaignOptions) =>
  executeCampaign({ environment: providerEnvironment, ...options });

const setup = async (): Promise<string> => {
  const root = await mkdtemp(join(tmpdir(), 'mosaic-campaign-'));
  const bundle = 'generated bundle\n';
  const bundleChecksum = createHash('sha256').update(bundle).digest('hex');
  await mkdir(join(root, 'dist'), { recursive: true });
  await writeFile(join(root, 'dist', 'mosaic-bench-acp.mjs'), bundle);
  await Promise.all(
    ['mosaic', 'mosaic-direct'].map(async (agent) => {
      const directory = join(root, 'agents', agent);
      await mkdir(directory, { recursive: true });
      await writeFile(
        join(directory, 'manifest.toml'),
        `name = "${agent}"\nBF_BUNDLE_SHA256=${bundleChecksum}\n`,
      );
    }),
  );
  await writeFile(
    join(root, 'skillsbench-report.json'),
    JSON.stringify({
      benchmark: 'skillsbench',
      valid: true,
      reasons: [],
      direct: fullMetrics(0.5, 2, 8700),
      mosaic: fullMetrics(0.8, 1, 8600),
      paired: fullPaired(true),
      paretoWin: true,
      exitCode: 0,
    }),
  );
  return root;
};

const digest = async (root: string): Promise<string> =>
  createHash('sha256')
    .update(await readFile(join(root, 'dist', 'mosaic-bench-acp.mjs')))
    .digest('hex');

const artifactRunner = async (
  root: string,
  command: Command,
): Promise<CommandResult> => {
  if (command.args[0] === 'ls-remote') {
    const commit = command.args[1]?.includes('terminal-bench-2')
      ? terminalCommit
      : skillsCommit;
    return { code: 0, stdout: `${commit}\trefs/tags/pinned\n`, stderr: '' };
  }
  if (command.file === 'curl' && command.args.at(-1)?.endsWith('.sha256'))
    return {
      code: 0,
      stdout: `${await digest(root)}  mosaic-bench-acp.mjs\n`,
      stderr: '',
    };
  if (command.file === 'uvx' && command.args.includes('--from')) {
    await writeArtifacts(command);
  }
  return { code: 0, stdout: 'ok\n', stderr: '' };
};

const writeArtifacts = async (command: Command): Promise<void> => {
  const value = (flag: string): string =>
    command.args[command.args.indexOf(flag) + 1]!;
  const agent = value('--agent');
  const skillMode = value('--skill-mode');
  const expected = Number(value('--expected-tasks'));
  const task = command.args.includes('tasks/jax-computing-basics')
    ? 'jax-computing-basics'
    : 'regex-log';
  const manifest = {
    schema_version: 1,
    total: expected,
    tasks: [{ task_id: task }],
  };
  const runConfig = {
    schema_version: 1,
    eval: {
      agent,
      model: 'openrouter/openai/gpt-5.6-luna',
      reasoning_effort: null,
      environment: 'docker',
      concurrency: 1,
      build_concurrency: 1,
      skill_mode: skillMode,
      usage_tracking: { requested: 'required' },
    },
    retry_attempts: 0,
  };
  const health = {
    schema_version: 1,
    total_rows: expected,
    scored_rows: expected,
    unscored_rows: 0,
    missing_llm_trajectory: 0,
    malformed_llm_trajectory: 0,
    rows: Array.from({ length: expected }, () => ({
      task_id: task,
      scored: true,
      error: null,
      verifier_error: null,
    })),
  };
  await Promise.all([
    writeFile(value('--task-manifest-out'), JSON.stringify(manifest)),
    writeFile(value('--run-config-out'), JSON.stringify(runConfig)),
    writeFile(value('--health-summary-out'), JSON.stringify(health)),
  ]);
};

const paidCommands = (commands: readonly Command[]): readonly Command[] =>
  commands.filter(
    (command) => command.file === 'uvx' && command.args.includes('--from'),
  );

test('assembles both smoke arms after a verified preflight', async (t) => {
  const root = await setup();
  t.after(() => rm(root, { recursive: true, force: true }));
  const commands: Command[] = [];
  const result = await campaign({
    benchmark: 'skillsbench',
    action: 'smoke',
    rootDir: root,
    yesPaidRun: true,
    runner: async (command) => {
      commands.push(command);
      return artifactRunner(root, command);
    },
  });
  assert.equal('arms' in result, true);
  if (!('arms' in result)) return;
  assert.deepEqual(
    paidCommands(commands).map((command) => command.args),
    result.arms.map((arm) => [
      '--from',
      'benchflow==0.6.5',
      'bench',
      'eval',
      'run',
      '--source-repo',
      'benchflow-ai/skillsbench',
      '--source-path',
      'tasks/jax-computing-basics',
      '--source-ref',
      skillsCommit,
      '--agent',
      arm.arm,
      '--model',
      'openrouter/openai/gpt-5.6-luna',
      '--sandbox',
      'docker',
      '--concurrency',
      '1',
      '--build-concurrency',
      '1',
      '--retry-attempts',
      '0',
      '--loop-strategy',
      'single-shot',
      '--usage-tracking',
      'required',
      '--skill-mode',
      'with-skill',
      '--jobs-dir',
      `${arm.directory}/jobs`,
      '--task-manifest-out',
      `${arm.directory}/task-manifest.json`,
      '--run-config-out',
      `${arm.directory}/run-config.json`,
      '--health-summary-out',
      `${arm.directory}/health.json`,
      '--expected-tasks',
      '1',
    ]),
  );
  assert.equal(
    commands.some(
      (command) =>
        command.file === 'curl' && command.args.at(-1)?.endsWith('.sha256'),
    ),
    true,
  );
  const metadata = JSON.parse(
    await readFile(join(result.arms[0]!.directory, 'metadata.json'), 'utf8'),
  );
  assert.deepEqual(
    {
      action: metadata.action,
      benchflowVersion: metadata.benchflowVersion,
      campaignId: metadata.campaignId,
      effort: metadata.effort,
      sandbox: metadata.sandbox,
      concurrency: metadata.concurrency,
      buildConcurrency: metadata.buildConcurrency,
      retries: metadata.retries,
      loopStrategy: metadata.loopStrategy,
      usageTracking: metadata.usageTracking,
    },
    {
      action: 'smoke',
      benchflowVersion: '0.6.5',
      campaignId: basename(result.directory),
      effort: 'low',
      sandbox: 'docker',
      concurrency: 1,
      buildConcurrency: 1,
      retries: 0,
      loopStrategy: 'single-shot',
      usageTracking: 'required',
    },
  );
  assert.deepEqual(Object.keys(metadata.digests).sort(), [
    'agentManifest',
    'bundle',
    'health',
    'runConfig',
    'taskManifest',
  ]);
  const secondMetadata = JSON.parse(
    await readFile(join(result.arms[1]!.directory, 'metadata.json'), 'utf8'),
  );
  assert.deepEqual(
    [secondMetadata.campaignId, secondMetadata.action],
    [metadata.campaignId, metadata.action],
  );
  await Promise.all(
    ['agent-manifest.toml', 'bundle.mjs'].map((file) =>
      stat(join(result.arms[0]!.directory, file)),
    ),
  );
});

test('assembles the fixed ten-task SkillsBench pilot for both arms', async (t) => {
  const root = await setup();
  t.after(() => rm(root, { recursive: true, force: true }));
  const commands: Command[] = [];
  const result = await campaign({
    benchmark: 'skillsbench',
    action: 'pilot',
    rootDir: root,
    yesPaidRun: true,
    runner: async (command) => {
      commands.push(command);
      return artifactRunner(root, command);
    },
  });
  assert.equal('arms' in result, true);
  if (!('arms' in result)) return;

  for (const command of paidCommands(commands)) {
    const includeTasks = command.args.flatMap((value, index) =>
      value === '--include' ? [command.args[index + 1]!] : [],
    );
    assert.equal(
      command.args[command.args.indexOf('--source-path') + 1],
      'tasks',
    );
    assert.equal(
      command.args[command.args.indexOf('--expected-tasks') + 1],
      '10',
    );
    assert.deepEqual(includeTasks, pilotTasks);
  }
  assert.equal(result.arms.length, 2);
  const metadata = JSON.parse(
    await readFile(join(result.arms[0]!.directory, 'metadata.json'), 'utf8'),
  );
  assert.deepEqual(
    [metadata.action, metadata.source.path, metadata.expectedTasks],
    ['pilot', 'tasks', 10],
  );
});

test('rejects a Terminal-Bench pilot before commands or results', async (t) => {
  const root = await setup();
  t.after(() => rm(root, { recursive: true, force: true }));
  let calls = 0;
  await assert.rejects(
    campaign({
      benchmark: 'terminalbench',
      action: 'pilot',
      rootDir: root,
      yesPaidRun: true,
      skillsbenchReport: join(root, 'skillsbench-report.json'),
      runner: async () => {
        calls += 1;
        return { code: 0, stdout: '', stderr: '' };
      },
    }),
    /only available for SkillsBench/,
  );
  assert.equal(calls, 0);
  await assert.rejects(stat(join(root, 'results')));
});

test('rejects an unconfirmed paid run before every command', async (t) => {
  const root = await setup();
  t.after(() => rm(root, { recursive: true, force: true }));
  let calls = 0;
  await assert.rejects(
    campaign({
      benchmark: 'skillsbench',
      action: 'run',
      rootDir: root,
      runner: async () => {
        calls += 1;
        return { code: 0, stdout: '', stderr: '' };
      },
    }),
    /yesPaidRun/,
  );
  assert.equal(calls, 0);
});

test('checks the actual runner, assets, checksum, local bundle, and remote commit', async (t) => {
  const root = await setup();
  t.after(() => rm(root, { recursive: true, force: true }));
  const commands: Command[] = [];
  const result = await campaign({
    benchmark: 'skillsbench',
    action: 'check',
    rootDir: root,
    runner: async (command) => {
      commands.push(command);
      return artifactRunner(root, command);
    },
  });
  assert.equal('checks' in result, true);
  if (!('checks' in result)) return;
  assert.equal(result.ok, true);
  assert.equal(
    commands.some(
      (command) => command.file === 'uvx' && command.args[0] === '--version',
    ),
    true,
  );
  assert.equal(
    commands.some((command) => command.file === 'uv'),
    false,
  );
  assert.equal(
    commands.some(
      (command) => command.file === 'curl' && command.args[0] === '--version',
    ),
    true,
  );
  assert.equal(
    commands.some(
      (command) =>
        command.file === 'curl' &&
        command.args.at(-1)?.endsWith('/mosaic-bench-acp.mjs'),
    ),
    true,
  );
  assert.equal(
    commands.some(
      (command) =>
        command.file === 'curl' &&
        command.args.at(-1)?.endsWith('/mosaic-bench-acp.mjs.sha256'),
    ),
    true,
  );
  assert.equal(
    result.checks.find((check) => check.name === 'release-checksum')?.ok,
    true,
  );
  assert.equal(
    result.checks.find(
      (check) => check.name === 'credential:OPENROUTER_API_KEY',
    )?.ok,
    true,
  );
});

test('rejects a paid campaign without OPENROUTER_API_KEY before creating results', async (t) => {
  const root = await setup();
  t.after(() => rm(root, { recursive: true, force: true }));

  await assert.rejects(
    executeCampaign({
      benchmark: 'skillsbench',
      action: 'smoke',
      rootDir: root,
      yesPaidRun: true,
      environment: {},
      runner: (command) => artifactRunner(root, command),
    }),
    /preflight/,
  );
  await assert.rejects(stat(join(root, 'results')));
});

test('rejects a release sidecar that does not match the local bundle', async (t) => {
  const root = await setup();
  t.after(() => rm(root, { recursive: true, force: true }));
  const result = await campaign({
    benchmark: 'skillsbench',
    action: 'check',
    rootDir: root,
    runner: async (command) => {
      if (command.file === 'curl' && command.args.at(-1)?.endsWith('.sha256'))
        return {
          code: 0,
          stdout: `${'0'.repeat(64)}  mosaic-bench-acp.mjs\n`,
          stderr: '',
        };
      return artifactRunner(root, command);
    },
  });
  assert.equal('checks' in result, true);
  if (!('checks' in result)) return;
  assert.equal(result.ok, false);
  assert.equal(
    result.checks.find((check) => check.name === 'release-checksum')?.ok,
    false,
  );
});

test('rejects different bundle pins in the two manifests', async (t) => {
  const root = await setup();
  t.after(() => rm(root, { recursive: true, force: true }));
  await writeFile(
    join(root, 'agents', 'mosaic', 'manifest.toml'),
    `name = "mosaic"\nBF_BUNDLE_SHA256=${'0'.repeat(64)}\n`,
  );
  const result = await campaign({
    benchmark: 'skillsbench',
    action: 'check',
    rootDir: root,
    runner: (command) => artifactRunner(root, command),
  });
  assert.equal('checks' in result, true);
  if (!('checks' in result)) return;
  assert.equal(result.ok, false);
  assert.equal(
    result.checks.find((check) => check.name === 'release-checksum')?.detail,
    'manifest checksums missing or different',
  );
});

test('stops after a failed first paid arm', async (t) => {
  const root = await setup();
  t.after(() => rm(root, { recursive: true, force: true }));
  const commands: Command[] = [];
  const result = await campaign({
    benchmark: 'skillsbench',
    action: 'smoke',
    rootDir: root,
    yesPaidRun: true,
    runner: async (command) => {
      commands.push(command);
      if (command.file === 'uvx' && command.args.includes('--from'))
        return { code: 1, stdout: '', stderr: 'failed' };
      return artifactRunner(root, command);
    },
  });
  assert.equal('arms' in result, true);
  if (!('arms' in result)) return;
  assert.equal(result.arms.length, 1);
  assert.equal(result.arms[0]?.arm, 'mosaic-direct');
  assert.equal(paidCommands(commands).length, 1);
});

test('fails a paid preflight before creating results', async (t) => {
  const root = await setup();
  t.after(() => rm(root, { recursive: true, force: true }));
  await rm(join(root, 'dist', 'mosaic-bench-acp.mjs'));
  await assert.rejects(
    campaign({
      benchmark: 'skillsbench',
      action: 'smoke',
      rootDir: root,
      yesPaidRun: true,
      runner: (command) => artifactRunner(root, command),
    }),
    /preflight/,
  );
  await assert.rejects(stat(join(root, 'results')));
});

test('requires a valid SkillsBench report before a paid Terminal-Bench campaign', async (t) => {
  const root = await setup();
  t.after(() => rm(root, { recursive: true, force: true }));
  let calls = 0;
  await assert.rejects(
    campaign({
      benchmark: 'terminalbench',
      action: 'smoke',
      rootDir: root,
      yesPaidRun: true,
      runner: async () => {
        calls += 1;
        return { code: 0, stdout: '', stderr: '' };
      },
    }),
    /skillsbenchReport/,
  );
  assert.equal(calls, 0);
  const result = await campaign({
    benchmark: 'terminalbench',
    action: 'smoke',
    rootDir: root,
    yesPaidRun: true,
    skillsbenchReport: join(root, 'skillsbench-report.json'),
    runner: (command) => artifactRunner(root, command),
  });
  assert.equal('arms' in result, true);
});

test('rejects minimal and smoke SkillsBench reports before Terminal-Bench commands', async (t) => {
  const root = await setup();
  t.after(() => rm(root, { recursive: true, force: true }));
  const minimal = join(root, 'minimal.json');
  const smoke = join(root, 'smoke.json');
  await Promise.all([
    writeFile(
      minimal,
      JSON.stringify({ valid: true, benchmark: 'skillsbench' }),
    ),
    writeFile(
      smoke,
      JSON.stringify({
        benchmark: 'skillsbench',
        valid: true,
        reasons: [],
        direct: {
          score: 0.5,
          reward: 0.5,
          costUsd: 2,
          costPerRewardUsd: 4,
          totalTokens: 100,
          tasks: 1,
        },
        mosaic: {
          score: 0.8,
          reward: 0.8,
          costUsd: 1,
          costPerRewardUsd: 1.25,
          totalTokens: 90,
          tasks: 1,
        },
        paired: {
          scoreDelta: 0.3,
          qualityWin: true,
          mosaicWins: ['task'],
          regressions: [],
          ties: [],
        },
        paretoWin: true,
        exitCode: 0,
      }),
    ),
  ]);
  let calls = 0;
  const options = (skillsbenchReport: string) => ({
    benchmark: 'terminalbench' as const,
    action: 'smoke' as const,
    rootDir: root,
    yesPaidRun: true,
    skillsbenchReport,
    runner: async (): Promise<CommandResult> => {
      calls += 1;
      return { code: 0, stdout: '', stderr: '' };
    },
  });
  await assert.rejects(campaign(options(minimal)), /valid SkillsBench report/);
  await assert.rejects(campaign(options(smoke)), /valid SkillsBench report/);
  assert.equal(calls, 0);
});

test('accepts a full valid non-Pareto SkillsBench report for Terminal-Bench', async (t) => {
  const root = await setup();
  t.after(() => rm(root, { recursive: true, force: true }));
  const report = join(root, 'skillsbench-no-pareto.json');
  await writeFile(
    report,
    JSON.stringify({
      benchmark: 'skillsbench',
      valid: true,
      reasons: [],
      direct: fullMetrics(0.8, 1, 8700),
      mosaic: fullMetrics(0.5, 2, 8600),
      paired: fullPaired(false),
      paretoWin: false,
      exitCode: 1,
    }),
  );
  const result = await campaign({
    benchmark: 'terminalbench',
    action: 'smoke',
    rootDir: root,
    yesPaidRun: true,
    skillsbenchReport: report,
    runner: (command) => artifactRunner(root, command),
  });
  assert.equal('arms' in result, true);
});

test('creates a fresh campaign directory for every paid campaign', async (t) => {
  const root = await setup();
  t.after(() => rm(root, { recursive: true, force: true }));
  const options = {
    benchmark: 'skillsbench' as const,
    action: 'smoke' as const,
    rootDir: root,
    yesPaidRun: true,
    runner: (command: Command) => artifactRunner(root, command),
  };
  const [first, second] = await Promise.all([
    campaign(options),
    campaign(options),
  ]);
  assert.equal('directory' in first && 'directory' in second, true);
  if (!('directory' in first) || !('directory' in second)) return;
  assert.notEqual(first.directory, second.directory);
});
