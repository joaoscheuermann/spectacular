import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';
import {
  copyFile,
  mkdir,
  open,
  readFile,
  readdir,
  realpath,
  rename,
  stat,
  unlink,
  writeFile,
} from 'node:fs/promises';
import { basename, dirname, join, resolve } from 'node:path';

import {
  campaign,
  type Arm,
  type Benchmark,
  type CampaignMetadata,
  type CampaignRun,
  type Command,
  type CommandRunner,
} from './campaign.js';
import { skillsbenchPilotTasks } from './pilot.js';

export type ResumeOptions = {
  readonly benchmark: Benchmark;
  readonly campaignDir: string;
  readonly rootDir: string;
  readonly yesPaidRun?: boolean;
  readonly runner?: CommandRunner;
  readonly environment?: NodeJS.ProcessEnv;
};

type Json = Record<string, unknown>;
type Evidence = {
  readonly directory: string;
  readonly metadata: CampaignMetadata;
};

const model = 'openrouter/openai/gpt-5.6-luna';
const commit = 'b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af';
const arms = ['mosaic-direct', 'mosaic'] as const;
const pilotTaskNames = new Set<string>(skillsbenchPilotTasks);
const digestNames = [
  'taskManifest',
  'runConfig',
  'health',
  'bundle',
  'agentManifest',
] as const;
const metadataNames = [
  'action',
  'agent',
  'benchflowVersion',
  'benchmark',
  'buildConcurrency',
  'campaignId',
  'concurrency',
  'digests',
  'effort',
  'expectedTasks',
  'loopStrategy',
  'model',
  'retries',
  'sandbox',
  'skillMode',
  'source',
  'usageTracking',
] as const;

const processRunner =
  (environment: NodeJS.ProcessEnv): CommandRunner =>
  ({ file, args, cwd, env }) =>
    new Promise((resolveResult, reject) => {
      const child = spawn(file, args as string[], {
        cwd,
        env: { ...environment, ...env },
        stdio: ['ignore', 'pipe', 'pipe'],
      });
      let stdout = '';
      let stderr = '';
      child.stdout.on('data', (chunk: Buffer) => {
        stdout += chunk.toString();
      });
      child.stderr.on('data', (chunk: Buffer) => {
        stderr += chunk.toString();
      });
      child.once('error', reject);
      child.once('close', (code) =>
        resolveResult({ code: code ?? 1, stdout, stderr }),
      );
    });

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
  const evidence = await loadEvidence(root, options.campaignDir);
  const release = await acquire(evidence.directory);
  try {
    const runner =
      options.runner ?? processRunner(options.environment ?? process.env);
    const preflight = await campaign({
      benchmark: 'skillsbench',
      action: 'check',
      rootDir: root,
      runner,
      environment: options.environment,
    });
    if (!('ok' in preflight) || !preflight.ok)
      throw new Error('Benchmark preflight failed.');
    await requireResources(root, runner);

    const runs = [];
    for (const arm of arms) {
      const armRun = await resumeArm(root, evidence, arm, runner);
      runs.push(armRun);
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

const loadEvidence = async (
  root: string,
  campaignDir: string,
): Promise<Evidence> => {
  const [directory, resultsDir] = await Promise.all([
    realpath(resolve(campaignDir)),
    realpath(join(root, 'results')),
  ]).catch(() => {
    throw new Error('Campaign has invalid resume evidence.');
  });
  if (dirname(directory) !== resultsDir)
    throw new Error('Resume campaign must be a direct child of results/.');
  const direct = join(directory, 'mosaic-direct');
  const candidate = await loadMetadata(join(direct, 'metadata.json'));
  if (!validMetadata(candidate, basename(directory), 'mosaic-direct'))
    throw new Error('Campaign has invalid resume evidence.');
  const metadata = candidate;
  if (
    !(await validArtifacts(root, direct, metadata)) ||
    !(await validSelection(direct, 'mosaic-direct'))
  )
    throw new Error('Campaign has invalid resume evidence.');

  const mosaic = join(directory, 'mosaic');
  if (await isDirectory(mosaic)) {
    const mosaicMetadata = await loadMetadata(join(mosaic, 'metadata.json'));
    if (
      !validMetadata(mosaicMetadata, basename(directory), 'mosaic') ||
      !(await validArtifacts(root, mosaic, mosaicMetadata)) ||
      !(await validSelection(mosaic, 'mosaic'))
    )
      throw new Error('Campaign has invalid resume evidence.');
  }
  return { directory, metadata };
};

const loadMetadata = async (path: string): Promise<unknown> => {
  try {
    const value: unknown = JSON.parse(await readFile(path, 'utf8'));
    return value;
  } catch {
    throw new Error('Campaign has invalid resume evidence.');
  }
};

const validMetadata = (
  candidate: unknown,
  campaignId: string,
  agent: Arm,
): candidate is CampaignMetadata => {
  const metadata = record(candidate);
  const source = record(metadata.source);
  return (
    sameStrings(Object.keys(metadata).sort(), [...metadataNames].sort()) &&
    sameStrings(Object.keys(source).sort(), ['path', 'ref', 'repo']) &&
    metadata.action === 'pilot' &&
    metadata.benchmark === 'skillsbench' &&
    metadata.benchflowVersion === '0.6.5' &&
    metadata.campaignId === campaignId &&
    metadata.agent === agent &&
    metadata.expectedTasks === skillsbenchPilotTasks.length &&
    metadata.model === model &&
    metadata.effort === 'low' &&
    metadata.sandbox === 'docker' &&
    metadata.concurrency === 1 &&
    metadata.buildConcurrency === 1 &&
    metadata.retries === 0 &&
    metadata.loopStrategy === 'single-shot' &&
    metadata.usageTracking === 'required' &&
    metadata.skillMode === 'with-skill' &&
    source.repo === 'benchflow-ai/skillsbench' &&
    source.path === 'tasks' &&
    source.ref === commit
  );
};

const validArtifacts = async (
  root: string,
  directory: string,
  metadata: CampaignMetadata,
): Promise<boolean> => {
  const paths = {
    taskManifest: join(directory, 'task-manifest.json'),
    runConfig: join(directory, 'run-config.json'),
    health: join(directory, 'health.json'),
    bundle: join(directory, 'bundle.mjs'),
    agentManifest: join(directory, 'agent-manifest.toml'),
  };
  const expected = record(metadata.digests);
  const actual = await Promise.all(
    digestNames.map(async (name) => [name, await digest(paths[name])] as const),
  );
  const recorded =
    sameStrings(Object.keys(expected).sort(), [...digestNames].sort()) &&
    actual.every(
      ([name, value]) => value !== undefined && expected[name] === value,
    );
  if (!recorded) return false;

  const arm = metadata.agent as Arm;
  const [localBundle, localManifest] = await Promise.all([
    digest(join(root, 'dist', 'mosaic-bench-acp.mjs')),
    digest(join(root, 'agents', arm, 'manifest.toml')),
  ]);
  return (
    expected.bundle === localBundle && expected.agentManifest === localManifest
  );
};

const validSelection = async (
  directory: string,
  agent: Arm,
): Promise<boolean> => {
  const [manifest, config, resultNames, jobCount] = await Promise.all([
    json(join(directory, 'task-manifest.json')),
    json(join(directory, 'run-config.json')),
    results(join(directory, 'jobs')),
    directories(join(directory, 'jobs')),
  ]);
  const tasks = Array.isArray(manifest.tasks)
    ? manifest.tasks.map((task) => string(record(task).task_id))
    : [];
  const evaluation = record(config.eval);
  const include = strings(evaluation.include_tasks);
  const exclude = strings(evaluation.exclude_tasks);
  const usage = record(evaluation.usage_tracking);
  const manifestSource = record(manifest.source);
  const configSource = record(evaluation.source_provenance);
  const validResults =
    new Set(resultNames).size === resultNames.length &&
    resultNames.every((task) => pilotTaskNames.has(task));
  return (
    manifest.schema_version === 1 &&
    manifest.total === skillsbenchPilotTasks.length &&
    sameStrings(sorted(tasks), sorted(skillsbenchPilotTasks)) &&
    sourceEvidence(manifestSource) &&
    config.schema_version === 1 &&
    config.retry_attempts === 0 &&
    evaluation.agent === agent &&
    evaluation.model === model &&
    evaluation.reasoning_effort === null &&
    evaluation.environment === 'docker' &&
    evaluation.concurrency === 1 &&
    evaluation.build_concurrency === 1 &&
    evaluation.skill_mode === 'with-skill' &&
    usage.requested === 'required' &&
    sameStrings(strings(evaluation.agent_env_keys) ?? ['invalid'], []) &&
    evaluation.skills_dir === null &&
    include !== undefined &&
    sameStrings(sorted(include), sorted(skillsbenchPilotTasks)) &&
    exclude !== undefined &&
    sameStrings(exclude, []) &&
    evaluation.dataset_name === null &&
    evaluation.dataset_version === null &&
    sourceEvidence(configSource) &&
    jobCount === 1 &&
    validResults
  );
};

const sourceEvidence = (source: Json): boolean =>
  source.type === 'github' &&
  source.repo === 'benchflow-ai/skillsbench' &&
  source.requested_ref === commit &&
  source.resolved_sha === commit &&
  source.path === 'tasks' &&
  source.dirty === false;

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
  evidence: Evidence,
  arm: Arm,
  runner: CommandRunner,
) => {
  const directory = join(evidence.directory, arm);
  const existed = await isDirectory(directory);
  await mkdir(join(directory, 'jobs'), { recursive: true });
  const command = evalCommand(root, directory, arm);
  const result = await runner(command);
  if (!existed) await copyAssets(root, directory, arm);
  if (result.code === 0) await canonicalize(directory);
  const metadata: CampaignMetadata = {
    ...evidence.metadata,
    agent: arm,
    digests: {},
  };
  await writeMetadata(directory, metadata, result.code === 0);
  return { arm, directory, command, result };
};

const canonicalize = async (directory: string): Promise<void> => {
  const jobs = join(directory, 'jobs');
  const jobNames = await directoryNames(jobs);
  if (jobNames.length !== 1)
    throw new Error('Resumed arm did not retain one canonical job directory.');
  const jobName = jobNames[0]!;
  const job = join(jobs, jobName);
  const rollouts = await directoryNames(job);
  const records = await Promise.all(
    rollouts.map(async (name) => {
      const path = join(job, name, 'result.json');
      const result = await json(path);
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
  if (!sameStrings(sorted([...keep.keys()]), sorted(skillsbenchPilotTasks)))
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

const json = async (path: string): Promise<Json> => {
  try {
    const value: unknown = JSON.parse(await readFile(path, 'utf8'));
    return record(value);
  } catch {
    return {};
  }
};

const results = async (path: string): Promise<readonly string[]> => {
  try {
    const entries = await readdir(path, { withFileTypes: true });
    const nested = await Promise.all(
      entries.map(async (entry): Promise<readonly string[]> => {
        const child = join(path, entry.name);
        if (entry.isDirectory()) return results(child);
        if (entry.name !== 'result.json') return [];
        return [string((await json(child)).task_name)].filter(Boolean);
      }),
    );
    return nested.flat();
  } catch {
    return [];
  }
};

const directories = async (path: string): Promise<number> => {
  try {
    return (await readdir(path, { withFileTypes: true })).filter((entry) =>
      entry.isDirectory(),
    ).length;
  } catch {
    return 0;
  }
};

const directoryNames = async (path: string): Promise<readonly string[]> => {
  try {
    return (await readdir(path, { withFileTypes: true }))
      .filter((entry) => entry.isDirectory())
      .map((entry) => entry.name)
      .sort();
  } catch {
    return [];
  }
};

const digest = async (path: string): Promise<string | undefined> => {
  try {
    return createHash('sha256')
      .update(await readFile(path))
      .digest('hex');
  } catch {
    return undefined;
  }
};

const isDirectory = async (path: string): Promise<boolean> => {
  try {
    return (await stat(path)).isDirectory();
  } catch {
    return false;
  }
};

const record = (value: unknown): Json =>
  typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as Json)
    : {};
const string = (value: unknown): string =>
  typeof value === 'string' ? value : '';
const strings = (value: unknown): readonly string[] | undefined =>
  Array.isArray(value) && value.every((item) => typeof item === 'string')
    ? value
    : undefined;
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
