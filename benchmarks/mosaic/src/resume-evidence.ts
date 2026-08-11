import { createHash } from 'node:crypto';
import { readFile, readdir, realpath, stat } from 'node:fs/promises';
import { basename, dirname, join, resolve } from 'node:path';

import { armOrder, type Arm, type CampaignMetadata } from './campaign-types.js';
import { skillsbenchPilotTasks } from './pilot.js';

export type Json = Record<string, unknown>;
export type ResumeEvidence = {
  readonly directory: string;
  readonly metadata: CampaignMetadata;
};

export const model = 'openrouter/openai/gpt-5.6-luna';
export const commit = 'b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af';
export const pilotTaskNames = new Set<string>(skillsbenchPilotTasks);
export const digestNames = [
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

/** Loads only closed, digest-matched evidence from one pilot campaign. */
export const loadEvidence = async (
  root: string,
  campaignDir: string,
): Promise<ResumeEvidence> => {
  const [directory, resultsDir] = await Promise.all([
    realpath(resolve(campaignDir)),
    realpath(join(root, 'results')),
  ]).catch(() => {
    throw new Error('Campaign has invalid resume evidence.');
  });
  if (dirname(directory) !== resultsDir)
    throw new Error('Resume campaign must be a direct child of results/.');
  const candidates = await Promise.all(
    armOrder.map(async (arm) => {
      const armDirectory = join(directory, arm);
      if (!(await isDirectory(armDirectory))) return undefined;
      const candidate = await loadMetadata(join(armDirectory, 'metadata.json'));
      if (
        !validMetadata(candidate, basename(directory), arm) ||
        !(await validArtifacts(root, armDirectory, candidate)) ||
        !(await validSelection(armDirectory, arm))
      )
        throw new Error('Campaign has invalid resume evidence.');
      return candidate;
    }),
  );
  const metadata = candidates.find(
    (candidate): candidate is CampaignMetadata => candidate !== undefined,
  );
  if (metadata === undefined)
    throw new Error('Campaign has invalid resume evidence.');
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
    readJson(join(directory, 'task-manifest.json')),
    readJson(join(directory, 'run-config.json')),
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
    resultNames.every((task) => pilotTaskNames.has(task))
  );
};

const sourceEvidence = (source: Json): boolean =>
  source.type === 'github' &&
  source.repo === 'benchflow-ai/skillsbench' &&
  source.requested_ref === commit &&
  source.resolved_sha === commit &&
  source.path === 'tasks' &&
  source.dirty === false;

export const readJson = async (path: string): Promise<Json> => {
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
        return [string((await readJson(child)).task_name)].filter(Boolean);
      }),
    );
    return nested.flat();
  } catch {
    return [];
  }
};

const directories = async (path: string): Promise<number> =>
  (await directoryNames(path)).length;

export const directoryNames = async (
  path: string,
): Promise<readonly string[]> => {
  try {
    return (await readdir(path, { withFileTypes: true }))
      .filter((entry) => entry.isDirectory())
      .map((entry) => entry.name)
      .sort();
  } catch {
    return [];
  }
};

export const digest = async (path: string): Promise<string | undefined> => {
  try {
    return createHash('sha256')
      .update(await readFile(path))
      .digest('hex');
  } catch {
    return undefined;
  }
};

export const isDirectory = async (path: string): Promise<boolean> => {
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
