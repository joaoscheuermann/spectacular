import {
  copyFile,
  mkdir,
  open,
  readFile,
  rename,
  stat,
  unlink,
  writeFile,
} from 'node:fs/promises';
import { join, resolve } from 'node:path';

import {
  campaign,
  type Arm,
  type Benchmark,
  type CampaignMetadata,
  type CampaignRun,
  type Command,
  type CommandRunner,
} from './campaign.js';
import { armOrder } from './campaign-types.js';
import { skillsbenchPilotTasks } from './pilot.js';
import { createProcessRunner } from './process.js';
import {
  commit,
  digest,
  digestNames,
  directoryNames,
  isDirectory,
  loadEvidence,
  model,
  pilotTaskNames,
  readJson,
  type ResumeEvidence,
} from './resume-evidence.js';

export type ResumeOptions = {
  readonly benchmark: Benchmark;
  readonly campaignDir: string;
  readonly rootDir: string;
  readonly yesPaidRun?: boolean;
  readonly runner?: CommandRunner;
  readonly environment?: NodeJS.ProcessEnv;
  readonly progress?: (stage: string) => void;
};

/** Resumes a validated SkillsBench pilot without changing its campaign identity. */
export const resumeCampaign = async (
  options: ResumeOptions,
): Promise<CampaignRun> => {
  if (options.yesPaidRun !== true)
    throw new Error('Paid benchmark resumes require yesPaidRun: true.');
  if (options.benchmark !== 'skillsbench')
    throw new Error(
      'Resume is currently available only for SkillsBench pilots.',
    );

  const root = resolve(options.rootDir);
  options.progress?.('validating campaign evidence');
  const evidence = await loadEvidence(root, options.campaignDir);
  const release = await acquire(evidence.directory);
  try {
    const runner =
      options.runner ?? createProcessRunner(options.environment ?? process.env);
    options.progress?.('running preflight checks');
    const preflight = await campaign({
      benchmark: 'skillsbench',
      action: 'check',
      rootDir: root,
      runner,
      environment: options.environment,
    });
    if (!('ok' in preflight) || !preflight.ok)
      throw new Error('Benchmark preflight failed.');
    options.progress?.('checking Docker resources');
    await requireResources(root, runner);

    const runs = [];
    for (const arm of armOrder) {
      options.progress?.(`running ${arm}`);
      const armRun = await resumeArm(root, evidence, arm, runner);
      runs.push(armRun);
      options.progress?.(`${arm} exited with code ${armRun.result.code}`);
      if (armRun.result.code !== 0) break;
    }
    return { directory: evidence.directory, arms: runs };
  } finally {
    await release();
  }
};

const acquire = async (directory: string): Promise<() => Promise<void>> => {
  const path = join(directory, '.resume.lock');
  try {
    const handle = await open(path, 'wx');
    try {
      await handle.writeFile(`${process.pid}\n`);
    } catch (error) {
      await unlink(path).catch(() => undefined);
      throw error;
    } finally {
      await handle.close();
    }
  } catch (error) {
    if (!hasCode(error, 'EEXIST')) throw error;
    const pid = Number((await readFile(path, 'utf8').catch(() => '')).trim());
    if (Number.isSafeInteger(pid) && running(pid))
      throw new Error('A resume is already in progress for this campaign.');
    await unlink(path).catch(() => undefined);
    return acquire(directory);
  }
  return () => unlink(path).catch(() => undefined);
};

const running = (pid: number): boolean => {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return !hasCode(error, 'ESRCH');
  }
};

const requireResources = async (
  root: string,
  runner: CommandRunner,
): Promise<void> => {
  const result = await runner({
    file: 'docker',
    args: ['info', '--format', '{{.NCPU}} {{.MemTotal}}'],
    cwd: root,
  });
  const [cpuText, memoryText] = result.stdout.trim().split(/\s+/, 2);
  const cpus = Number(cpuText);
  const memory = Number(memoryText);
  if (result.code !== 0 || cpus < 8 || memory < 8 * 1024 ** 3)
    throw new Error(
      `SkillsBench pilot resume requires at least 8 CPUs and 8 GiB of Docker memory; found ${cpus || 0} CPUs and ${Math.floor((memory || 0) / 1024 ** 3)} GiB.`,
    );
};

const resumeArm = async (
  root: string,
  evidence: ResumeEvidence,
  arm: Arm,
  runner: CommandRunner,
) => {
  const directory = join(evidence.directory, arm);
  const existed = await isDirectory(directory);
  await mkdir(join(directory, 'jobs'), { recursive: true });
  if (existed) await canonicalize(directory, false);
  const command = evalCommand(root, directory, arm);
  const result = await runner(command);
  if (!existed) await copyAssets(root, directory, arm);
  await canonicalize(directory, result.code === 0);
  const metadata: CampaignMetadata = {
    ...evidence.metadata,
    agent: arm,
    digests: {},
  };
  await writeMetadata(directory, metadata, result.code === 0);
  return { arm, directory, command, result };
};

const canonicalize = async (
  directory: string,
  requireComplete: boolean,
): Promise<void> => {
  const jobs = join(directory, 'jobs');
  const jobNames = await directoryNames(jobs);
  if (jobNames.length !== 1 && requireComplete)
    throw new Error('Resumed arm did not retain one canonical job directory.');
  if (jobNames.length !== 1) return;
  const jobName = jobNames[0]!;
  const job = join(jobs, jobName);
  const rollouts = await directoryNames(job);
  const records = await Promise.all(
    rollouts.map(async (name) => {
      const path = join(job, name, 'result.json');
      const result = await readJson(path);
      const details = await stat(path).catch(() => undefined);
      return {
        name,
        task: string(result.task_name),
        modified: details?.mtimeMs ?? -1,
      };
    }),
  );
  const keep = new Map<string, (typeof records)[number]>();
  for (const record of records) {
    if (!record.task) continue;
    const current = keep.get(record.task);
    if (
      current === undefined ||
      record.modified > current.modified ||
      (record.modified === current.modified && record.name > current.name)
    )
      keep.set(record.task, record);
  }
  const tasks = [...keep.keys()];
  const knownTasks = tasks.every((task) => pilotTaskNames.has(task));
  const complete = sameStrings(sorted(tasks), sorted(skillsbenchPilotTasks));
  if (!knownTasks || (requireComplete && !complete))
    throw new Error('Resumed arm did not produce one result per pilot task.');

  const retained = new Set([...keep.values()].map((record) => record.name));
  const superseded = rollouts.filter((name) => !retained.has(name));
  if (superseded.length === 0) return;
  const archive = join(directory, 'attempts', jobName);
  await mkdir(archive, { recursive: true });
  await Promise.all(
    superseded.map((name) => rename(join(job, name), join(archive, name))),
  );
};

const evalCommand = (root: string, directory: string, arm: Arm): Command => ({
  file: 'uvx',
  args: [
    '--from',
    'benchflow==0.6.5',
    'bench',
    'eval',
    'run',
    '--source-repo',
    'benchflow-ai/skillsbench',
    '--source-path',
    'tasks',
    '--source-ref',
    commit,
    '--agent',
    arm,
    '--model',
    model,
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
    ...skillsbenchPilotTasks.flatMap((task) => ['--include', task]),
    '--jobs-dir',
    join(directory, 'jobs'),
    '--task-manifest-out',
    join(directory, 'task-manifest.json'),
    '--run-config-out',
    join(directory, 'run-config.json'),
    '--health-summary-out',
    join(directory, 'health.json'),
    '--expected-tasks',
    String(skillsbenchPilotTasks.length),
  ],
  cwd: root,
  env: { BENCHFLOW_AGENTS_DIR: join(root, 'agents') },
});

const copyAssets = async (
  root: string,
  directory: string,
  arm: Arm,
): Promise<void> =>
  Promise.all([
    copyFile(
      join(root, 'agents', arm, 'manifest.toml'),
      join(directory, 'agent-manifest.toml'),
    ),
    copyFile(
      join(root, 'dist', 'mosaic-bench-acp.mjs'),
      join(directory, 'bundle.mjs'),
    ),
  ]).then(() => undefined);

const writeMetadata = async (
  directory: string,
  metadata: CampaignMetadata,
  requireAll: boolean,
): Promise<void> => {
  const paths = {
    taskManifest: join(directory, 'task-manifest.json'),
    runConfig: join(directory, 'run-config.json'),
    health: join(directory, 'health.json'),
    bundle: join(directory, 'bundle.mjs'),
    agentManifest: join(directory, 'agent-manifest.toml'),
  };
  const entries = await Promise.all(
    digestNames.map(async (name) => [name, await digest(paths[name])] as const),
  );
  const digests = Object.fromEntries(
    entries.filter(
      (entry): entry is readonly [(typeof digestNames)[number], string] =>
        entry[1] !== undefined,
    ),
  );
  if (requireAll && Object.keys(digests).length !== digestNames.length)
    throw new Error('Successful resumed arm did not produce every artifact.');
  await writeFile(
    join(directory, 'metadata.json'),
    `${JSON.stringify({ ...metadata, digests }, null, 2)}\n`,
  );
};

const string = (value: unknown): string =>
  typeof value === 'string' ? value : '';
const sorted = (values: readonly string[]): readonly string[] =>
  [...values].sort();
const sameStrings = (
  left: readonly string[],
  right: readonly string[],
): boolean => JSON.stringify(left) === JSON.stringify(right);
const hasCode = (error: unknown, code: string): boolean =>
  typeof error === 'object' &&
  error !== null &&
  'code' in error &&
  error.code === code;
