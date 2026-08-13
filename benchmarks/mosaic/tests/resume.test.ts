import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import {
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  realpath,
  rename,
  rm,
  stat,
  writeFile,
} from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import type { Command, CommandResult } from '../src/campaign.js';
import { skillsbenchPilotTasks } from '../src/pilot.js';
import { resumeCampaign } from '../src/resume.js';

const skillsCommit = 'b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af';
const environment = { OPENROUTER_API_KEY: 'test-key' };
const hash = (value: string): string =>
  createHash('sha256').update(value).digest('hex');
const value = (command: Command, flag: string): string =>
  command.args[command.args.indexOf(flag) + 1]!;

const setup = async (): Promise<{
  readonly root: string;
  readonly directory: string;
  readonly bundleHash: string;
}> => {
  const root = await mkdtemp(join(tmpdir(), 'mosaic-resume-'));
  const directory = join(root, 'results', 'skillsbench-pilot-fixture');
  const direct = join(directory, 'mosaic-direct');
  const bundle = 'released bundle\n';
  const bundleHash = hash(bundle);
  const directManifest = `name = "mosaic-direct"\nBF_BUNDLE_SHA256=${bundleHash}\n`;
  const manifest = `name = "mosaic"\nBF_BUNDLE_SHA256=${bundleHash}\n`;
  const source = {
    type: 'github',
    repo: 'benchflow-ai/skillsbench',
    requested_ref: skillsCommit,
    resolved_sha: skillsCommit,
    path: 'tasks',
    dirty: false,
    file_hashes: {},
  };
  const taskManifest = JSON.stringify({
    schema_version: 1,
    total: 10,
    source,
    tasks: skillsbenchPilotTasks.map((task) => ({ task_id: task })),
  });
  const runConfig = JSON.stringify({
    schema_version: 1,
    eval: {
      agent: 'mosaic-direct',
      model: 'openrouter/deepseek/deepseek-v4-pro-0813',
      reasoning_effort: null,
      environment: 'docker',
      concurrency: 1,
      build_concurrency: 1,
      skill_mode: 'with-skill',
      usage_tracking: { requested: 'required' },
      agent_env_keys: [],
      skills_dir: null,
      include_tasks: skillsbenchPilotTasks,
      exclude_tasks: [],
      source_provenance: source,
      dataset_name: null,
      dataset_version: null,
    },
    retry_attempts: 0,
  });
  const health = JSON.stringify({ schema_version: 1, total_rows: 10 });
  const metadata = {
    action: 'pilot',
    benchmark: 'skillsbench',
    benchflowVersion: '0.6.5',
    campaignId: 'skillsbench-pilot-fixture',
    source: {
      repo: 'benchflow-ai/skillsbench',
      path: 'tasks',
      ref: skillsCommit,
    },
    expectedTasks: 10,
    agent: 'mosaic-direct',
    model: 'openrouter/deepseek/deepseek-v4-pro-0813',
    effort: 'low',
    sandbox: 'docker',
    concurrency: 1,
    buildConcurrency: 1,
    retries: 0,
    loopStrategy: 'single-shot',
    usageTracking: 'required',
    skillMode: 'with-skill',
    digests: {
      taskManifest: hash(taskManifest),
      runConfig: hash(runConfig),
      health: hash(health),
      bundle: bundleHash,
      agentManifest: hash(directManifest),
    },
  };

  await Promise.all([
    mkdir(join(root, 'dist'), { recursive: true }),
    mkdir(join(root, 'agents', 'mosaic-direct'), { recursive: true }),
    mkdir(join(root, 'agents', 'mosaic'), { recursive: true }),
    mkdir(join(direct, 'jobs', '2026-08-10__21-24-13'), { recursive: true }),
  ]);
  await Promise.all([
    writeFile(join(root, 'dist', 'mosaic-bench-acp.mjs'), bundle),
    writeFile(
      join(root, 'agents', 'mosaic-direct', 'manifest.toml'),
      directManifest,
    ),
    writeFile(join(root, 'agents', 'mosaic', 'manifest.toml'), manifest),
    writeFile(join(direct, 'bundle.mjs'), bundle),
    writeFile(join(direct, 'agent-manifest.toml'), directManifest),
    writeFile(join(direct, 'task-manifest.json'), taskManifest),
    writeFile(join(direct, 'run-config.json'), runConfig),
    writeFile(join(direct, 'health.json'), health),
    writeFile(join(direct, 'metadata.json'), JSON.stringify(metadata)),
  ]);
  await Promise.all(
    skillsbenchPilotTasks.map((task, index) =>
      mkdir(join(direct, 'jobs', '2026-08-10__21-24-13', `${task}__${index}`), {
        recursive: true,
      }),
    ),
  );
  await Promise.all(
    skillsbenchPilotTasks.map((task, index) =>
      writeFile(
        join(
          direct,
          'jobs',
          '2026-08-10__21-24-13',
          `${task}__${index}`,
          'result.json',
        ),
        JSON.stringify({
          task_name: task,
          ...(index < 8
            ? { rewards: { reward: index < 3 ? 1 : 0 }, error: null }
            : { rewards: null, error: 'docker resources' }),
        }),
      ),
    ),
  );
  return {
    root: await realpath(root),
    directory: await realpath(directory),
    bundleHash,
  };
};

const keepOnlyMosaicEvidence = async (
  root: string,
  directory: string,
): Promise<void> => {
  const direct = join(directory, 'mosaic-direct');
  const mosaic = join(directory, 'mosaic');
  const runConfig = JSON.parse(
    await readFile(join(direct, 'run-config.json'), 'utf8'),
  );
  runConfig.eval.agent = 'mosaic';
  const runConfigText = JSON.stringify(runConfig);
  const manifest = await readFile(
    join(root, 'agents', 'mosaic', 'manifest.toml'),
    'utf8',
  );
  const metadata = JSON.parse(
    await readFile(join(direct, 'metadata.json'), 'utf8'),
  );
  metadata.agent = 'mosaic';
  metadata.digests.runConfig = hash(runConfigText);
  metadata.digests.agentManifest = hash(manifest);
  await Promise.all([
    writeFile(join(direct, 'run-config.json'), runConfigText),
    writeFile(join(direct, 'agent-manifest.toml'), manifest),
    writeFile(join(direct, 'metadata.json'), JSON.stringify(metadata)),
  ]);
  await rename(direct, mosaic);
};

const writeOutputs = async (command: Command): Promise<void> => {
  const agent = value(command, '--agent');
  const source = {
    type: 'github',
    repo: 'benchflow-ai/skillsbench',
    requested_ref: skillsCommit,
    resolved_sha: skillsCommit,
    path: 'tasks',
    dirty: false,
    file_hashes: {},
  };
  await Promise.all([
    writeFile(
      value(command, '--task-manifest-out'),
      JSON.stringify({
        schema_version: 1,
        total: 10,
        source,
        tasks: skillsbenchPilotTasks.map((task) => ({ task_id: task })),
      }),
    ),
    writeFile(
      value(command, '--run-config-out'),
      JSON.stringify({
        schema_version: 1,
        eval: {
          agent,
          model: 'openrouter/deepseek/deepseek-v4-pro-0813',
          reasoning_effort: null,
          environment: 'docker',
          concurrency: 1,
          build_concurrency: 1,
          skill_mode: 'with-skill',
          usage_tracking: { requested: 'required' },
          agent_env_keys: [],
          skills_dir: null,
          include_tasks: skillsbenchPilotTasks,
          exclude_tasks: [],
          source_provenance: source,
          dataset_name: null,
          dataset_version: null,
        },
        retry_attempts: 0,
      }),
    ),
    writeFile(
      value(command, '--health-summary-out'),
      JSON.stringify({ schema_version: 1, total_rows: 10 }),
    ),
  ]);

  const job = join(value(command, '--jobs-dir'), '2026-08-10__21-24-13');
  const existingJob = await stat(job)
    .then(() => true)
    .catch(() => false);
  const tasks =
    agent === 'mosaic-direct' && existingJob
      ? skillsbenchPilotTasks.slice(8)
      : skillsbenchPilotTasks;
  await Promise.all(
    tasks.map(async (task, index) => {
      const rollout = join(job, `${task}__resumed-${index}`);
      await mkdir(rollout, { recursive: true });
      await writeFile(
        join(rollout, 'result.json'),
        JSON.stringify({
          task_name: task,
          rewards: { reward: 1 },
          error: null,
        }),
      );
    }),
  );
};

const runner =
  (
    root: string,
    bundleHash: string,
    commands: Command[],
    resources = '8 10737418240',
  ) =>
  async (command: Command): Promise<CommandResult> => {
    commands.push(command);
    if (command.args[0] === 'ls-remote')
      return {
        code: 0,
        stdout: `${skillsCommit}\trefs/tags/pinned\n`,
        stderr: '',
      };
    if (command.file === 'curl' && command.args.at(-1)?.endsWith('.sha256'))
      return {
        code: 0,
        stdout: `${bundleHash}  mosaic-bench-acp.mjs\n`,
        stderr: '',
      };
    if (
      command.file === 'docker' &&
      command.args.includes('{{.NCPU}} {{.MemTotal}}')
    )
      return { code: 0, stdout: `${resources}\n`, stderr: '' };
    if (command.file === 'uvx' && command.args.includes('--from'))
      await writeOutputs(command);
    void root;
    return { code: 0, stdout: 'ok\n', stderr: '' };
  };

test('runs Mosaic before resuming the existing direct jobs directory', async (t) => {
  const { root, directory, bundleHash } = await setup();
  t.after(() => rm(root, { recursive: true, force: true }));
  const commands: Command[] = [];
  const result = await resumeCampaign({
    benchmark: 'skillsbench',
    campaignDir: directory,
    rootDir: root,
    yesPaidRun: true,
    environment,
    runner: runner(root, bundleHash, commands),
  });
  const paid = commands.filter(
    (command) => command.file === 'uvx' && command.args.includes('--from'),
  );

  assert.equal(result.directory, directory);
  assert.deepEqual(
    result.arms.map((arm) => arm.arm),
    ['mosaic', 'mosaic-direct'],
  );
  assert.equal(
    value(paid[0]!, '--jobs-dir'),
    join(directory, 'mosaic', 'jobs'),
  );
  assert.equal(
    value(paid[1]!, '--jobs-dir'),
    join(directory, 'mosaic-direct', 'jobs'),
  );
  assert.deepEqual(
    paid[0]!.args.flatMap((entry, index) =>
      entry === '--include' ? [paid[0]!.args[index + 1]!] : [],
    ),
    skillsbenchPilotTasks,
  );
  await stat(join(directory, 'mosaic', 'metadata.json'));
  const directMetadata = JSON.parse(
    await readFile(join(directory, 'mosaic-direct', 'metadata.json'), 'utf8'),
  );
  const mosaicMetadata = JSON.parse(
    await readFile(join(directory, 'mosaic', 'metadata.json'), 'utf8'),
  );
  assert.deepEqual(
    [
      directMetadata.campaignId,
      mosaicMetadata.campaignId,
      mosaicMetadata.action,
    ],
    ['skillsbench-pilot-fixture', 'skillsbench-pilot-fixture', 'pilot'],
  );
  const directRollouts = await readdir(
    join(directory, 'mosaic-direct', 'jobs', '2026-08-10__21-24-13'),
  );
  const archivedAttempts = await readdir(
    join(directory, 'mosaic-direct', 'attempts', '2026-08-10__21-24-13'),
  );
  assert.equal(directRollouts.length, skillsbenchPilotTasks.length);
  assert.deepEqual(archivedAttempts.sort(), [
    `${skillsbenchPilotTasks[8]}__8`,
    `${skillsbenchPilotTasks[9]}__9`,
  ]);
});

test('resumes from Mosaic-only evidence after the first arm failed', async (t) => {
  const { root, directory, bundleHash } = await setup();
  t.after(() => rm(root, { recursive: true, force: true }));
  await keepOnlyMosaicEvidence(root, directory);
  const commands: Command[] = [];

  const result = await resumeCampaign({
    benchmark: 'skillsbench',
    campaignDir: directory,
    rootDir: root,
    yesPaidRun: true,
    environment,
    runner: runner(root, bundleHash, commands),
  });

  assert.deepEqual(
    result.arms.map((arm) => arm.arm),
    ['mosaic', 'mosaic-direct'],
  );
  await stat(join(directory, 'mosaic-direct', 'metadata.json'));
});

test('rejects a second resume while the campaign lock is active', async (t) => {
  const { root, directory } = await setup();
  t.after(() => rm(root, { recursive: true, force: true }));
  await writeFile(join(directory, '.resume.lock'), `${process.pid}\n`);
  let calls = 0;

  await assert.rejects(
    resumeCampaign({
      benchmark: 'skillsbench',
      campaignDir: directory,
      rootDir: root,
      yesPaidRun: true,
      environment,
      runner: async () => {
        calls += 1;
        return { code: 0, stdout: '', stderr: '' };
      },
    }),
    /already in progress/,
  );
  assert.equal(calls, 0);
});

test('rejects insufficient Docker resources before a paid command', async (t) => {
  const { root, directory, bundleHash } = await setup();
  t.after(() => rm(root, { recursive: true, force: true }));
  const commands: Command[] = [];

  await assert.rejects(
    resumeCampaign({
      benchmark: 'skillsbench',
      campaignDir: directory,
      rootDir: root,
      yesPaidRun: true,
      environment,
      runner: runner(root, bundleHash, commands, '2 2147483648'),
    }),
    /at least 8 CPUs and 8 GiB/,
  );
  assert.equal(
    commands.some(
      (command) => command.file === 'uvx' && command.args.includes('--from'),
    ),
    false,
  );
});

test('consolidates duplicate attempts when a resumed arm still fails', async (t) => {
  const { root, directory, bundleHash } = await setup();
  t.after(() => rm(root, { recursive: true, force: true }));
  const job = join(directory, 'mosaic-direct', 'jobs', '2026-08-10__21-24-13');
  const duplicate = join(job, `${skillsbenchPilotTasks[8]}__duplicate`);
  await mkdir(duplicate);
  await writeFile(
    join(duplicate, 'result.json'),
    JSON.stringify({
      task_name: skillsbenchPilotTasks[8],
      rewards: null,
      error: 'previous retry',
    }),
  );
  const commands: Command[] = [];
  const execute = runner(root, bundleHash, commands);

  const result = await resumeCampaign({
    benchmark: 'skillsbench',
    campaignDir: directory,
    rootDir: root,
    yesPaidRun: true,
    environment,
    runner: async (command) => {
      const completed = await execute(command);
      return command.file === 'uvx' &&
        command.args.includes('--from') &&
        value(command, '--agent') === 'mosaic-direct'
        ? { ...completed, code: 1 }
        : completed;
    },
  });

  assert.equal(result.arms.length, 2);
  assert.equal(result.arms[0]?.arm, 'mosaic');
  assert.equal(result.arms[0]?.result.code, 0);
  assert.equal(result.arms[1]?.arm, 'mosaic-direct');
  assert.equal(result.arms[1]?.result.code, 1);
  assert.equal((await readdir(job)).length, skillsbenchPilotTasks.length);
  assert.equal(
    (
      await readdir(
        join(directory, 'mosaic-direct', 'attempts', '2026-08-10__21-24-13'),
      )
    ).length,
    3,
  );
});

test('rejects campaign evidence changed after its recorded digest', async (t) => {
  const { root, directory } = await setup();
  t.after(() => rm(root, { recursive: true, force: true }));
  await writeFile(join(directory, 'mosaic-direct', 'bundle.mjs'), 'tampered');
  let calls = 0;

  await assert.rejects(
    resumeCampaign({
      benchmark: 'skillsbench',
      campaignDir: directory,
      rootDir: root,
      yesPaidRun: true,
      environment,
      runner: async () => {
        calls += 1;
        return { code: 0, stdout: '', stderr: '' };
      },
    }),
    /invalid resume evidence/,
  );
  assert.equal(calls, 0);
});
