import { createHash, randomUUID } from 'node:crypto';
import {
  copyFile,
  mkdir,
  mkdtemp,
  readFile,
  stat,
  writeFile,
} from 'node:fs/promises';
import { basename, join, resolve } from 'node:path';
import { spawn } from 'node:child_process';

import { isValidSkillsbenchReport } from './compare.js';

export type Benchmark = 'skillsbench' | 'terminalbench';
export type CampaignAction = 'check' | 'smoke' | 'run';
export type Arm = 'mosaic-direct' | 'mosaic';

export type Command = {
  readonly file: string;
  readonly args: readonly string[];
  readonly cwd: string;
  readonly env?: Readonly<Record<string, string>>;
};

export type CommandResult = {
  readonly code: number;
  readonly stdout: string;
  readonly stderr: string;
};
export type CommandRunner = (command: Command) => Promise<CommandResult>;

export type Check = {
  readonly name: string;
  readonly ok: boolean;
  readonly detail: string;
};
export type CampaignCheck = {
  readonly ok: boolean;
  readonly checks: readonly Check[];
};
export type CampaignMetadata = {
  readonly action: 'smoke' | 'run';
  readonly benchmark: Benchmark;
  readonly benchflowVersion: '0.6.5';
  readonly campaignId: string;
  readonly source: {
    readonly repo: string;
    readonly path: string;
    readonly ref: string;
  };
  readonly expectedTasks: number;
  readonly agent: Arm;
  readonly model: string;
  readonly effort: 'low';
  readonly sandbox: 'docker';
  readonly concurrency: 1;
  readonly buildConcurrency: 1;
  readonly retries: 0;
  readonly loopStrategy: 'single-shot';
  readonly usageTracking: 'required';
  readonly skillMode: 'with-skill' | 'no-skill';
  readonly digests: Readonly<Record<string, string>>;
};
export type ArmRun = {
  readonly arm: Arm;
  readonly directory: string;
  readonly command: Command;
  readonly result: CommandResult;
};
export type CampaignRun = {
  readonly directory: string;
  readonly arms: readonly ArmRun[];
};
export type CampaignOptions = {
  readonly benchmark: Benchmark;
  readonly action: CampaignAction;
  readonly rootDir: string;
  readonly yesPaidRun?: boolean;
  readonly skillsbenchReport?: string;
  readonly runner?: CommandRunner;
};

const model = 'openai/gpt-5.6-luna';
const agents = ['mosaic-direct', 'mosaic'] as const;
const releaseAssets = [
  'https://github.com/joaoscheuermann/spectacular/releases/download/mosaic-benchmark-v0.1.0/mosaic-bench-acp.mjs',
  'https://github.com/joaoscheuermann/spectacular/releases/download/mosaic-benchmark-v0.1.0/mosaic-bench-acp.mjs.sha256',
] as const;

const definitions: Record<
  Benchmark,
  {
    readonly repo: string;
    readonly ref: string;
    readonly fullPath: string;
    readonly smokePath: string;
    readonly expected: number;
    readonly skillMode: 'with-skill' | 'no-skill';
  }
> = {
  skillsbench: {
    repo: 'benchflow-ai/skillsbench',
    ref: 'b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af',
    fullPath: 'tasks',
    smokePath: 'tasks/edit-pdf',
    expected: 87,
    skillMode: 'with-skill',
  },
  terminalbench: {
    repo: 'laude-institute/terminal-bench-2',
    ref: '2fd12b88aafdd04a52c298e3940bcb189f9766d6',
    fullPath: '.',
    smokePath: 'regex-log',
    expected: 89,
    skillMode: 'no-skill',
  },
};

const processRunner: CommandRunner = ({ file, args, cwd, env }) =>
  new Promise((resolveResult, reject) => {
    const child = spawn(file, args as string[], {
      cwd,
      env: { ...process.env, ...env },
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

/** Runs the free preflight or creates an explicitly approved paid campaign. */
export const campaign = async (
  options: CampaignOptions,
): Promise<CampaignCheck | CampaignRun> => {
  const runner = options.runner ?? processRunner;
  return options.action === 'check'
    ? check(options, runner)
    : run(options, runner);
};

const check = async (
  options: CampaignOptions,
  runner: CommandRunner,
): Promise<CampaignCheck> => {
  const definition = definitions[options.benchmark];
  const root = resolve(options.rootDir);
  const commands: readonly Check[] = await Promise.all([
    executable('uvx', runner, root),
    executable('docker', runner, root),
    executable('git', runner, root),
    executable('curl', runner, root),
    commandCheck(
      'docker-daemon',
      {
        file: 'docker',
        args: ['info', '--format', '{{.ServerVersion}}'],
        cwd: root,
      },
      runner,
    ),
    remoteRef(definition.repo, definition.ref, runner, root),
    ...releaseAssets.map((asset) =>
      commandCheck(
        `release-asset:${asset}`,
        { file: 'curl', args: ['-fsSL', '-o', '/dev/null', asset], cwd: root },
        runner,
      ),
    ),
    releaseChecksum(root, runner),
  ]);
  const files = await Promise.all([
    ...agents.map((arm) =>
      fileCheck(`manifest:${arm}`, join(root, 'agents', arm, 'manifest.toml')),
    ),
    fileCheck(
      'bundle:mosaic-bench-acp.mjs',
      join(root, 'dist', 'mosaic-bench-acp.mjs'),
    ),
  ]);
  const checks = [...commands, ...files];
  return { ok: checks.every((item) => item.ok), checks };
};

const executable = (name: string, runner: CommandRunner, cwd: string) =>
  commandCheck(name, { file: name, args: ['--version'], cwd }, runner);
const remoteRef = async (
  repo: string,
  ref: string,
  runner: CommandRunner,
  cwd: string,
): Promise<Check> => {
  try {
    const result = await runner({
      file: 'git',
      args: ['ls-remote', `https://github.com/${repo}.git`],
      cwd,
    });
    const found = result.code === 0 && checkRemoteRef(ref, result.stdout);
    return {
      name: `remote-ref:${repo}`,
      ok: found,
      detail: found ? 'ok' : 'pinned commit not advertised',
    };
  } catch {
    return { name: `remote-ref:${repo}`, ok: false, detail: 'not available' };
  }
};
const checkRemoteRef = (ref: string, output: string): boolean =>
  output.split('\n').some((line) => line.split('\t', 1)[0] === ref);
const releaseChecksum = async (
  root: string,
  runner: CommandRunner,
): Promise<Check> => {
  const bundle = join(root, 'dist', 'mosaic-bench-acp.mjs');
  const local = await digest(bundle);
  const pinned = await manifestBundleChecksum(root);
  if (local === undefined)
    return {
      name: 'release-checksum',
      ok: false,
      detail: 'local bundle missing',
    };
  if (pinned === undefined)
    return {
      name: 'release-checksum',
      ok: false,
      detail: 'manifest checksums missing or different',
    };
  try {
    const result = await runner({
      file: 'curl',
      args: ['-fsSL', releaseAssets[1]],
      cwd: root,
    });
    const declared = result.stdout.trim().split(/\s+/, 1)[0];
    const ok = result.code === 0 && local === pinned && declared === pinned;
    return {
      name: 'release-checksum',
      ok,
      detail: ok ? 'ok' : 'release checksum differs',
    };
  } catch {
    return { name: 'release-checksum', ok: false, detail: 'not available' };
  }
};
const manifestBundleChecksum = async (
  root: string,
): Promise<string | undefined> => {
  const checksums = await Promise.all(
    agents.map(async (arm) => {
      try {
        const source = await readFile(
          join(root, 'agents', arm, 'manifest.toml'),
          'utf8',
        );
        return /^BF_BUNDLE_SHA256=([a-f0-9]{64})$/im.exec(source)?.[1];
      } catch {
        return undefined;
      }
    }),
  );
  const [first] = checksums;
  return first !== undefined && checksums.every((value) => value === first)
    ? first
    : undefined;
};
const commandCheck = async (
  name: string,
  command: Command,
  runner: CommandRunner,
): Promise<Check> => {
  try {
    const result = await runner(command);
    return {
      name,
      ok: result.code === 0,
      detail: result.code === 0 ? 'ok' : `exit ${result.code}`,
    };
  } catch {
    return { name, ok: false, detail: 'not available' };
  }
};
const fileCheck = async (name: string, path: string): Promise<Check> => {
  try {
    return { name, ok: (await stat(path)).isFile(), detail: 'ok' };
  } catch {
    return { name, ok: false, detail: 'missing' };
  }
};

const run = async (
  options: CampaignOptions,
  runner: CommandRunner,
): Promise<CampaignRun> => {
  if (options.yesPaidRun !== true)
    throw new Error('Paid benchmark runs require yesPaidRun: true.');
  if (options.benchmark === 'terminalbench')
    await requireSkillsbenchReport(options.skillsbenchReport);
  const preflight = await check(options, runner);
  if (!preflight.ok) throw new Error('Benchmark preflight failed.');
  const definition = definitions[options.benchmark];
  const root = resolve(options.rootDir);
  const resultsDir = join(root, 'results');
  await mkdir(resultsDir, { recursive: true });
  const directory = await mkdtemp(
    join(resultsDir, `${options.benchmark}-${options.action}-${randomUUID()}-`),
  );
  const action = options.action === 'smoke' ? 'smoke' : 'run';
  const campaignId = basename(directory);
  const expectedTasks = action === 'smoke' ? 1 : definition.expected;
  const sourcePath =
    action === 'smoke' ? definition.smokePath : definition.fullPath;
  const arms: ArmRun[] = [];
  for (const arm of agents) {
    const armDirectory = join(directory, arm);
    await mkdir(join(armDirectory, 'jobs'), { recursive: true });
    const command = evalCommand(
      root,
      armDirectory,
      arm,
      definition,
      sourcePath,
      expectedTasks,
    );
    const result = await runner(command);
    await copyArmAssets(root, armDirectory, arm);
    await writeMetadata(
      armDirectory,
      metadata(
        action,
        campaignId,
        options.benchmark,
        definition,
        arm,
        expectedTasks,
        sourcePath,
      ),
      result.code === 0,
    );
    arms.push({ arm, directory: armDirectory, command, result });
    if (result.code !== 0) break;
  }
  return { directory, arms };
};

const evalCommand = (
  root: string,
  directory: string,
  arm: Arm,
  definition: (typeof definitions)[Benchmark],
  sourcePath: string,
  expectedTasks: number,
): Command => ({
  file: 'uvx',
  args: [
    '--from',
    'benchflow==0.6.5',
    'bench',
    'eval',
    'run',
    '--source-repo',
    definition.repo,
    '--source-path',
    sourcePath,
    '--source-ref',
    definition.ref,
    '--agent',
    arm,
    '--model',
    model,
    '--reasoning-effort',
    'low',
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
    definition.skillMode,
    '--jobs-dir',
    join(directory, 'jobs'),
    '--task-manifest-out',
    join(directory, 'task-manifest.json'),
    '--run-config-out',
    join(directory, 'run-config.json'),
    '--health-summary-out',
    join(directory, 'health.json'),
    '--expected-tasks',
    String(expectedTasks),
  ],
  cwd: root,
  env: { BENCHFLOW_AGENTS_DIR: join(root, 'agents') },
});

const metadata = (
  action: 'smoke' | 'run',
  campaignId: string,
  benchmark: Benchmark,
  definition: (typeof definitions)[Benchmark],
  agent: Arm,
  expectedTasks: number,
  sourcePath: string,
): CampaignMetadata => ({
  action,
  benchmark,
  benchflowVersion: '0.6.5',
  campaignId,
  source: { repo: definition.repo, path: sourcePath, ref: definition.ref },
  expectedTasks,
  agent,
  model,
  effort: 'low',
  sandbox: 'docker',
  concurrency: 1,
  buildConcurrency: 1,
  retries: 0,
  loopStrategy: 'single-shot',
  usageTracking: 'required',
  skillMode: definition.skillMode,
  digests: {},
});

const requireSkillsbenchReport = async (
  path: string | undefined,
): Promise<void> => {
  if (path === undefined)
    throw new Error('Terminal-Bench campaigns require skillsbenchReport.');
  try {
    const report: unknown = JSON.parse(await readFile(path, 'utf8'));
    if (!isValidSkillsbenchReport(report))
      throw new Error('invalid SkillsBench report');
  } catch {
    throw new Error(
      'Terminal-Bench campaigns require a valid SkillsBench report.',
    );
  }
};

const copyArmAssets = async (
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
  value: CampaignMetadata,
  requireAll: boolean,
): Promise<void> => {
  const candidates = {
    taskManifest: join(directory, 'task-manifest.json'),
    runConfig: join(directory, 'run-config.json'),
    health: join(directory, 'health.json'),
    bundle: join(directory, 'bundle.mjs'),
    agentManifest: join(directory, 'agent-manifest.toml'),
  };
  const entries = await Promise.all(
    Object.entries(candidates).map(
      async ([name, path]) => [name, await digest(path)] as const,
    ),
  );
  const digests = Object.fromEntries(
    entries.filter(
      (entry): entry is readonly [string, string] => entry[1] !== undefined,
    ),
  );
  if (
    requireAll &&
    Object.keys(digests).length !== Object.keys(candidates).length
  )
    throw new Error('Successful arm did not produce every required artifact.');
  await writeFile(
    join(directory, 'metadata.json'),
    `${JSON.stringify({ ...value, digests }, null, 2)}\n`,
  );
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
