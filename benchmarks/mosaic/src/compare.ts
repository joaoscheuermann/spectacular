import { createHash } from 'node:crypto';
import { readdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

import type { Benchmark } from './campaign.js';
import { skillsbenchPilotTasks } from './pilot.js';

export type Metrics = {
  readonly score: number;
  readonly reward: number;
  readonly costUsd: number;
  readonly costPerRewardUsd: number | null;
  readonly totalTokens: number;
  readonly tasks: number;
};

export type PairedReadings = {
  readonly scoreDelta: number;
  readonly qualityWin: boolean;
  readonly mosaicWins: readonly string[];
  readonly regressions: readonly string[];
  readonly ties: readonly string[];
};

export type CompareReport = {
  readonly benchmark: Benchmark | null;
  readonly valid: boolean;
  readonly reasons: readonly string[];
  readonly direct: Metrics;
  readonly mosaic: Metrics;
  readonly paired: PairedReadings;
  readonly paretoWin: boolean;
  readonly exitCode: 0 | 1 | 2;
};

export type CompareOptions = {
  readonly directDir: string;
  readonly mosaicDir: string;
  readonly reportPath?: string;
};

/** Narrows persisted evidence to a complete primary-campaign report. */
export const isValidSkillsbenchReport = (
  value: unknown,
): value is CompareReport => {
  const report = record(value);
  const direct = record(report.direct);
  const mosaic = record(report.mosaic);
  const paired = record(report.paired);

  const metricsValid = (metrics: Json): boolean => {
    const reward = number(metrics.reward);
    const cost = number(metrics.costUsd);
    const costPerReward = number(metrics.costPerRewardUsd);
    const score = number(metrics.score);
    const tasks = number(metrics.tasks);

    return (
      arraysEqual(sortedKeys(metrics), [
        'costPerRewardUsd',
        'costUsd',
        'reward',
        'score',
        'tasks',
        'totalTokens',
      ]) &&
      unitInterval(metrics.score) &&
      positive(metrics.costUsd) &&
      positive(metrics.totalTokens) &&
      tasks === 87 &&
      reward !== undefined &&
      reward >= 0 &&
      reward <= tasks &&
      score !== undefined &&
      approximatelyEqual(score, reward / tasks) &&
      (reward === 0
        ? metrics.costPerRewardUsd === null
        : cost !== undefined &&
          costPerReward !== undefined &&
          positive(costPerReward) &&
          approximatelyEqual(costPerReward, cost / reward))
    );
  };
  const mosaicWins = stringArray(paired.mosaicWins);
  const regressions = stringArray(paired.regressions);
  const ties = stringArray(paired.ties);

  const pairedTasks = [
    ...(mosaicWins ?? []),
    ...(regressions ?? []),
    ...(ties ?? []),
  ];
  const scoreDelta = number(paired.scoreDelta);
  const directScore = number(direct.score);
  const mosaicScore = number(mosaic.score);
  const qualityWin =
    scoreDelta !== undefined && scoreDelta > 0 && report.valid === true;

  const pairedValid =
    arraysEqual(sortedKeys(paired), [
      'mosaicWins',
      'qualityWin',
      'regressions',
      'scoreDelta',
      'ties',
    ]) &&
    scoreDelta !== undefined &&
    directScore !== undefined &&
    mosaicScore !== undefined &&
    approximatelyEqual(scoreDelta, mosaicScore - directScore) &&
    paired.qualityWin === qualityWin &&
    mosaicWins !== undefined &&
    regressions !== undefined &&
    ties !== undefined &&
    pairedTasks.every((task) => task.length > 0) &&
    new Set(pairedTasks).size === 87 &&
    pairedTasks.length === 87;

  const pareto =
    number(mosaic.score)! > number(direct.score)! &&
    number(mosaic.costUsd)! < number(direct.costUsd)!;

  return (
    arraysEqual(sortedKeys(report), [
      'benchmark',
      'direct',
      'exitCode',
      'mosaic',
      'paired',
      'paretoWin',
      'reasons',
      'valid',
    ]) &&
    report.benchmark === 'skillsbench' &&
    report.valid === true &&
    Array.isArray(report.reasons) &&
    report.reasons.length === 0 &&
    metricsValid(direct) &&
    metricsValid(mosaic) &&
    pairedValid &&
    report.paretoWin === pareto &&
    report.exitCode === (qualityWin ? 0 : 1)
  );
};

type Json = Record<string, unknown>;

type Artifact = { readonly value: Json; readonly digest?: string };

type ArmCampaign = {
  readonly metadata: Json;
  readonly taskManifest: Artifact;
  readonly runConfig: Artifact;
  readonly health: Artifact;
  readonly agentManifest?: string;
  readonly bundle?: string;
  readonly results: readonly Json[];
};

const empty: Metrics = {
  score: 0,
  reward: 0,
  costUsd: 0,
  costPerRewardUsd: null,
  totalTokens: 0,
  tasks: 0,
};
const model = 'openrouter/deepseek/deepseek-v4-pro';

const digestNames = [
  'taskManifest',
  'runConfig',
  'health',
  'bundle',
  'agentManifest',
] as const;

/** Validates paired evidence and reports quality, efficiency, and Pareto readings. */
export const compare = async (
  options: CompareOptions,
): Promise<CompareReport> => {
  const [direct, mosaic] = await Promise.all([
    load(options.directDir),
    load(options.mosaicDir),
  ]);
  const benchmark = benchmarkOf(direct.metadata);

  const reasons = [
    ...armIssues('direct', direct, 'mosaic-direct'),
    ...armIssues('mosaic', mosaic, 'mosaic'),
    ...pairIssues(direct, mosaic),
  ];
  const directMetrics = metrics(direct.results);
  const mosaicMetrics = metrics(mosaic.results);
  const valid = reasons.length === 0;
  const paired = pairedReadings(direct.results, mosaic.results, valid);

  const paretoWin =
    valid &&
    mosaicMetrics.score > directMetrics.score &&
    mosaicMetrics.costUsd < directMetrics.costUsd;

  const report: CompareReport = {
    benchmark,
    valid,
    reasons,
    direct: directMetrics,
    mosaic: mosaicMetrics,
    paired,
    paretoWin,
    exitCode: valid ? (paired.qualityWin ? 0 : 1) : 2,
  };

  if (options.reportPath)
    {await writeFile(options.reportPath, `${JSON.stringify(report, null, 2)}\n`);}

  return report;
};

const load = async (directory: string): Promise<ArmCampaign> => {
  const [taskManifest, runConfig, health, agentManifest, bundle, results] =
    await Promise.all([
      jsonArtifact(join(directory, 'task-manifest.json')),
      jsonArtifact(join(directory, 'run-config.json')),
      jsonArtifact(join(directory, 'health.json')),
      digest(join(directory, 'agent-manifest.toml')),
      digest(join(directory, 'bundle.mjs')),
      resultFiles(join(directory, 'jobs')),
    ]);

  return {
    metadata: await json(join(directory, 'metadata.json')),
    taskManifest,
    runConfig,
    health,
    agentManifest,
    bundle,
    results,
  };
};

const jsonArtifact = async (path: string): Promise<Artifact> => ({
  value: await json(path),
  digest: await digest(path),
});

const json = async (path: string): Promise<Json> => {
  try {
    const value: unknown = JSON.parse(await readFile(path, 'utf8'));

    return record(value);
  } catch {
    return {};
  }
};

const resultFiles = async (path: string): Promise<readonly Json[]> => {
  try {
    const entries = await readdir(path, { withFileTypes: true });

    const nested = await Promise.all(
      entries.map(
        (entry): Promise<readonly Json[] | Json | undefined> =>
          entry.isDirectory()
            ? resultFiles(join(path, entry.name))
            : entry.name === 'result.json'
              ? json(join(path, entry.name))
              : Promise.resolve(undefined),
      ),
    );

    return nested.flatMap((entry) =>
      Array.isArray(entry) ? entry : entry ? [entry] : [],
    );
  } catch {
    return [];
  }
};

const armIssues = (
  name: string,
  campaign: ArmCampaign,
  arm: 'mosaic-direct' | 'mosaic',
): readonly string[] => [
  ...metadataIssues(name, campaign.metadata, arm),
  ...digestIssues(name, campaign),
  ...manifestIssues(name, campaign),
  ...runConfigIssues(name, campaign),
  ...healthIssues(name, campaign),
  ...resultIssues(name, campaign),
];

const metadataIssues = (
  name: string,
  metadata: Json,
  arm: string,
): readonly string[] => {
  const benchmark = benchmarkOf(metadata);
  const expected = number(metadata.expectedTasks);
  const source = record(metadata.source);

  const validKeys = [
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
  ];

  const fixed =
    isCampaignAction(metadata.action) &&
    metadata.agent === arm &&
    metadata.benchflowVersion === '0.6.5' &&
    metadata.model === model &&
    metadata.effort === 'low' &&
    metadata.sandbox === 'docker' &&
    metadata.concurrency === 1 &&
    metadata.buildConcurrency === 1 &&
    metadata.retries === 0 &&
    metadata.loopStrategy === 'single-shot' &&
    metadata.usageTracking === 'required' &&
    string(metadata.campaignId).length > 0 &&
    arraysEqual(sortedKeys(metadata), validKeys);
  const sourceKeys = arraysEqual(sortedKeys(source), ['path', 'ref', 'repo']);

  return [
    ...(fixed ? [] : [`${name}: invalid metadata`]),
    ...(sourceKeys ? [] : [`${name}: invalid source metadata`]),
    ...(validSource(
      benchmark,
      source,
      expected,
      metadata.skillMode,
      metadata.action,
    )
      ? []
      : [`${name}: source is not a pinned benchmark contract`]),
  ];
};

const validSource = (
  benchmark: Benchmark | null,
  source: Json,
  expected: number | undefined,
  skillMode: unknown,
  action: unknown,
): boolean => {
  if (benchmark === 'skillsbench')
    {return (
      source.repo === 'benchflow-ai/skillsbench' &&
      source.ref === 'b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af' &&
      skillMode === 'with-skill' &&
      ((action === 'run' && source.path === 'tasks' && expected === 87) ||
        (action === 'pilot' &&
          source.path === 'tasks' &&
          expected === skillsbenchPilotTasks.length) ||
        (action === 'smoke' &&
          source.path === 'tasks/jax-computing-basics' &&
          expected === 1))
    );}

  if (benchmark === 'terminalbench')
    {return (
      source.repo === 'laude-institute/terminal-bench-2' &&
      source.ref === '2fd12b88aafdd04a52c298e3940bcb189f9766d6' &&
      skillMode === 'no-skill' &&
      ((action === 'run' && source.path === '.' && expected === 89) ||
        (action === 'smoke' && source.path === 'regex-log' && expected === 1))
    );}

  return false;
};

const sourceContract = (
  source: Json,
  metadataSource: Json,
  requireFileHashes: boolean,
): boolean => {
  const hashes = sourceFileHashes(source);

  return (
    source.type === 'github' &&
    source.repo === metadataSource.repo &&
    source.requested_ref === metadataSource.ref &&
    source.resolved_sha === metadataSource.ref &&
    source.path === metadataSource.path &&
    source.dirty === false &&
    hashes !== undefined &&
    validFileHashes(hashes, requireFileHashes)
  );
};

const taskSourceMatches = (taskSource: Json, parentSource: Json): boolean => {
  const parentPath = string(parentSource.path).replace(/^\.|\/$/g, '');
  const taskPath = string(taskSource.path).replace(/^\.|\/$/g, '');
  const hashes = sourceFileHashes(taskSource);

  return (
    taskSource.type === parentSource.type &&
    taskSource.repo === parentSource.repo &&
    taskSource.requested_ref === parentSource.requested_ref &&
    taskSource.resolved_sha === parentSource.resolved_sha &&
    taskSource.dirty === false &&
    (parentPath === '' ||
      taskPath === parentPath ||
      taskPath.startsWith(`${parentPath}/`)) &&
    hashes !== undefined &&
    validFileHashes(hashes, true)
  );
};

const sameSourceEvidence = (left: Json, right: Json): boolean => {
  const leftHashes = sourceFileHashes(left);
  const rightHashes = sourceFileHashes(right);

  return (
    left.type === right.type &&
    left.repo === right.repo &&
    left.requested_ref === right.requested_ref &&
    left.resolved_sha === right.resolved_sha &&
    left.path === right.path &&
    left.dirty === right.dirty &&
    leftHashes !== undefined &&
    rightHashes !== undefined &&
    sameFileHashes(leftHashes, rightHashes)
  );
};

const validFileHashes = (hashes: Json, required: boolean): boolean =>
  (!required || Object.keys(hashes).length > 0) &&
  Object.entries(hashes).every(
    ([name, digest]) => name.length > 0 && sha256Digest(digest),
  );

const sameFileHashes = (left: Json, right: Json): boolean =>
  arraysEqual(sortedKeys(left), sortedKeys(right)) &&
  Object.keys(left).every((key) => left[key] === right[key]);

const sourceNeedsTaskFiles = (metadata: Json): boolean =>
  metadata.action === 'smoke';

const digestIssues = (
  name: string,
  campaign: ArmCampaign,
): readonly string[] => {
  const expected = record(campaign.metadata.digests);

  const actual: Record<(typeof digestNames)[number], string | undefined> = {
    taskManifest: campaign.taskManifest.digest,
    runConfig: campaign.runConfig.digest,
    health: campaign.health.digest,
    bundle: campaign.bundle,
    agentManifest: campaign.agentManifest,
  };

  const issues = digestNames.filter(
    (key) => typeof actual[key] !== 'string' || expected[key] !== actual[key],
  );

  return [
    ...(arraysEqual(sortedKeys(expected), [...digestNames].sort())
      ? []
      : [`${name}: invalid digest metadata`]),
    ...issues.map((key) => `${name}: ${key} digest mismatch`),
  ];
};

const manifestIssues = (
  name: string,
  campaign: ArmCampaign,
): readonly string[] => {
  const manifest = campaign.taskManifest.value;
  const expected = number(campaign.metadata.expectedTasks);
  const tasks = manifest.tasks;
  const entries = Array.isArray(tasks) ? tasks.map(record) : [];
  const manifestIds = entries.map((task) => string(task.task_id));

  const resultNames = campaign.results.map((result) =>
    string(result.task_name),
  );

  const validNames =
    resultNames.length === expected &&
    resultNames.every(Boolean) &&
    new Set(resultNames).size === resultNames.length;

  const validManifestIds =
    manifestIds.length === expected &&
    manifestIds.every(Boolean) &&
    new Set(manifestIds).size === manifestIds.length &&
    entries.every(
      (task) =>
        sha256Digest(task.digest) && task.registry_digest_match !== false,
    );

  const valid =
    manifest.schema_version === 1 &&
    manifest.total === expected &&
    Array.isArray(tasks) &&
    tasks.length === expected &&
    validManifestIds &&
    validNames &&
    sourceContract(
      record(manifest.source),
      record(campaign.metadata.source),
      sourceNeedsTaskFiles(campaign.metadata),
    ) &&
    arraysEqual(sorted(manifestIds), sorted(resultNames));

  return valid ? [] : [`${name}: invalid task manifest`];
};

const runConfigIssues = (
  name: string,
  campaign: ArmCampaign,
): readonly string[] => {
  const config = campaign.runConfig.value;
  const evalConfig = record(config.eval);
  const usage = record(evalConfig.usage_tracking);
  const manifestSource = record(campaign.taskManifest.value.source);
  const source = record(evalConfig.source_provenance);
  const includeTasks = stringArray(evalConfig.include_tasks);
  const expectedIncludeTasks =
    campaign.metadata.action === 'pilot' ? skillsbenchPilotTasks : [];

  const valid =
    config.schema_version === 1 &&
    evalConfig.agent === campaign.metadata.agent &&
    evalConfig.model === model &&
    evalConfig.reasoning_effort === null &&
    evalConfig.environment === 'docker' &&
    evalConfig.concurrency === 1 &&
    evalConfig.build_concurrency === 1 &&
    evalConfig.skill_mode === campaign.metadata.skillMode &&
    arraysEqual(evalConfig.agent_env_keys as readonly string[], []) &&
    evalConfig.skills_dir === null &&
    includeTasks !== undefined &&
    arraysEqual(sorted(includeTasks), sorted(expectedIncludeTasks)) &&
    arraysEqual(evalConfig.exclude_tasks as readonly string[], []) &&
    evalConfig.dataset_name === null &&
    evalConfig.dataset_version === null &&
    usage.requested === 'required' &&
    config.retry_attempts === 0 &&
    sourceContract(
      source,
      record(campaign.metadata.source),
      sourceNeedsTaskFiles(campaign.metadata),
    ) &&
    sameSourceEvidence(source, manifestSource);

  return valid ? [] : [`${name}: invalid run config`];
};

const healthIssues = (
  name: string,
  campaign: ArmCampaign,
): readonly string[] => {
  const health = campaign.health.value;
  const expected = number(campaign.metadata.expectedTasks);
  const rows = health.rows;

  const resultsByTask = new Map(
    campaign.results.map((result) => [string(result.task_name), result]),
  );

  const resultTasks = sorted(
    campaign.results.map((result) => string(result.task_name)),
  );

  const healthTasks = Array.isArray(rows)
    ? sorted(rows.map((row) => string(record(row).task_id)))
    : [];

  const rowsValid =
    Array.isArray(rows) &&
    rows.length === expected &&
    rows.every((row) => {
      const value = record(row);
      const result = resultsByTask.get(string(value.task_id));

      return (
        value.scored === true &&
        hasNull(value, 'error') &&
        hasNull(value, 'verifier_error') &&
        unitInterval(value.reward) &&
        value.reward === record(result?.rewards).reward &&
        nonnegativeInteger(value.tool_calls) &&
        value.tool_calls === result?.n_tool_calls &&
        value.has_llm_trajectory === true &&
        value.valid_llm_trajectory === true &&
        positiveInteger(value.llm_trajectory_rows)
      );
    });

  const rowsWithToolCalls = Array.isArray(rows)
    ? rows.filter((row) => (number(record(row).tool_calls) ?? 0) > 0).length
    : -1;

  const zeroToolRows = Array.isArray(rows)
    ? rows.filter((row) => record(row).tool_calls === 0).length
    : -1;

  const valid =
    health.schema_version === 1 &&
    health.total_rows === expected &&
    health.scored_rows === expected &&
    health.unscored_rows === 0 &&
    health.missing_llm_trajectory === 0 &&
    health.malformed_llm_trajectory === 0 &&
    health.rows_with_tool_calls === rowsWithToolCalls &&
    health.zero_tool_rows === zeroToolRows &&
    rowsValid &&
    arraysEqual(healthTasks, resultTasks);

  return valid ? [] : [`${name}: invalid health summary`];
};

const resultIssues = (name: string, campaign: ArmCampaign): readonly string[] =>
  campaign.results.flatMap((result, index) => {
    const agent = record(result.agent_result);
    const loop = record(result.loop);
    const usage = record(result.usage_tracking);
    const taskDigest = manifestTaskDigest(campaign, string(result.task_name));
    const source = record(result.source);
    const reward = record(result.rewards).reward;

    const checks: readonly [string, boolean][] = [
      ['agent', result.agent === campaign.metadata.agent],
      ['agent_name', result.agent_name === 'mosaic-benchmark'],
      ['model', result.model === model],
      ['skill_mode', result.skill_mode === campaign.metadata.skillMode],
      ['loop.strategy', loop.strategy === 'single-shot'],
      [
        'usage_tracking',
        usage.requested === 'required' &&
          usage.status === 'enabled' &&
          usage.usage_source === agent.usage_source,
      ],
      ['trajectory_source', result.trajectory_source === 'acp'],
      ['partial_trajectory', result.partial_trajectory === false],
      ['n_prompts', result.n_prompts === 1],
      ['error', hasNull(result, 'error')],
      ['verifier_error', hasNull(result, 'verifier_error')],
      ['reward', unitInterval(reward)],
      ['n_tool_calls', nonnegativeInteger(result.n_tool_calls)],
      ['agent_result.n_tool_calls', agent.n_tool_calls === result.n_tool_calls],
      ['usage_source', agent.usage_source === 'provider_response'],
      ['price_source', agent.price_source === 'litellm'],
      ['total_tokens', positive(agent.total_tokens)],
      ['cost_usd', positive(agent.cost_usd)],
      [
        'task_digest',
        taskDigest !== undefined && result.task_digest === taskDigest,
      ],
      [
        'source',
        taskSourceMatches(source, record(campaign.taskManifest.value.source)),
      ],
    ];

    return checks
      .filter(([, ok]) => !ok)
      .map(([field]) => `${name}:${index}: invalid ${field}`);
  });

const manifestTaskDigest = (
  campaign: ArmCampaign,
  taskName: string,
): string | undefined => {
  const tasks = campaign.taskManifest.value.tasks;

  if (!Array.isArray(tasks)) {return undefined;}

  const task = tasks.map(record).find((entry) => entry.task_id === taskName);

  return task && sha256Digest(task.digest) ? task.digest : undefined;
};

const pairIssues = (
  direct: ArmCampaign,
  mosaic: ArmCampaign,
): readonly string[] => {
  const directTasks = sorted(
    direct.results.map((result) => string(result.task_name)),
  );

  const mosaicTasks = sorted(
    mosaic.results.map((result) => string(result.task_name)),
  );

  const sameSource =
    JSON.stringify(direct.metadata.source) ===
    JSON.stringify(mosaic.metadata.source);

  const sameManifest =
    direct.taskManifest.digest !== undefined &&
    direct.taskManifest.digest === mosaic.taskManifest.digest;

  const sameRunConfig =
    stableJson(normalizedRunConfig(direct.runConfig.value)) ===
    stableJson(normalizedRunConfig(mosaic.runConfig.value));

  const sameCampaignId =
    string(direct.metadata.campaignId).length > 0 &&
    direct.metadata.campaignId === mosaic.metadata.campaignId;

  const sameAction =
    isCampaignAction(direct.metadata.action) &&
    direct.metadata.action === mosaic.metadata.action;

  return [
    ...(benchmarkOf(direct.metadata) === benchmarkOf(mosaic.metadata)
      ? []
      : ['metadata differs: benchmark']),
    ...(sameSource ? [] : ['metadata differs: source']),
    ...(sameManifest ? [] : ['task manifests differ']),
    ...(sameRunConfig ? [] : ['run configs differ']),
    ...(direct.bundle !== undefined && direct.bundle === mosaic.bundle
      ? []
      : ['bundles differ']),
    ...(sameCampaignId ? [] : ['metadata differs: campaignId']),
    ...(sameAction ? [] : ['metadata differs: action']),
    ...(arraysEqual(directTasks, mosaicTasks) ? [] : ['task sets differ']),
  ];
};

const metrics = (items: readonly Json[]): Metrics => {
  if (items.length === 0) {return empty;}

  const reward = items.reduce(
    (sum, result) => sum + (number(record(result.rewards).reward) ?? 0),
    0,
  );

  const costUsd = items.reduce(
    (sum, result) => sum + (number(record(result.agent_result).cost_usd) ?? 0),
    0,
  );

  return {
    score: reward / items.length,
    reward,
    costUsd,
    costPerRewardUsd: reward > 0 ? costUsd / reward : null,
    totalTokens: items.reduce(
      (sum, result) =>
        sum + (number(record(result.agent_result).total_tokens) ?? 0),
      0,
    ),
    tasks: items.length,
  };
};

const pairedReadings = (
  direct: readonly Json[],
  mosaic: readonly Json[],
  valid: boolean,
): PairedReadings => {
  const rewards = (items: readonly Json[]): ReadonlyMap<string, number> =>
    new Map(
      items.flatMap((result) => {
        const task = string(result.task_name);
        const reward = number(record(result.rewards).reward);

        return task.length > 0 && reward !== undefined ? [[task, reward]] : [];
      }),
    );
  const directRewards = rewards(direct);
  const mosaicRewards = rewards(mosaic);

  const tasks = sorted([
    ...new Set([...directRewards.keys(), ...mosaicRewards.keys()]),
  ]);
  const mosaicWins: string[] = [];
  const regressions: string[] = [];
  const ties: string[] = [];

  for (const task of tasks) {
    const directReward = directRewards.get(task);
    const mosaicReward = mosaicRewards.get(task);

    if (directReward === undefined || mosaicReward === undefined) {continue;}

    if (mosaicReward > directReward) {mosaicWins.push(task);}
    else if (mosaicReward < directReward) {regressions.push(task);}
    else {ties.push(task);}
  }

  const directMetrics = metrics(direct);
  const mosaicMetrics = metrics(mosaic);
  const scoreDelta = rounded(mosaicMetrics.score - directMetrics.score);

  return {
    scoreDelta,
    qualityWin: valid && scoreDelta > 0,
    mosaicWins,
    regressions,
    ties,
  };
};

const normalizedRunConfig = (config: Json): Json => {
  const { jobs_dir: _jobsDir, eval: evalValue, ...rest } = config;
  const { agent: _agent, ...evalConfig } = record(evalValue);

  return { ...rest, eval: evalConfig };
};

const stableJson = (value: unknown): string => {
  if (Array.isArray(value)) {return `[${value.map(stableJson).join(',')}]`;}

  if (typeof value === 'object' && value !== null) {
    const object = value as Json;

    return `{${Object.keys(object)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${stableJson(object[key])}`)
      .join(',')}}`;
  }

  return JSON.stringify(value) ?? 'null';
};

const benchmarkOf = (metadata: Json): Benchmark | null =>
  metadata.benchmark === 'skillsbench' || metadata.benchmark === 'terminalbench'
    ? metadata.benchmark
    : null;

const digest = async (path: string): Promise<string | undefined> => {
  try {
    return createHash('sha256')
      .update(await readFile(path))
      .digest('hex');
  } catch {
    return undefined;
  }
};

const record = (value: unknown): Json =>
  typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as Json)
    : {};

const sourceFileHashes = (source: Json): Json | undefined =>
  Object.hasOwn(source, 'file_hashes') &&
  typeof source.file_hashes === 'object' &&
  source.file_hashes !== null &&
  !Array.isArray(source.file_hashes)
    ? (source.file_hashes as Json)
    : undefined;

const number = (value: unknown): number | undefined =>
  typeof value === 'number' && Number.isFinite(value) ? value : undefined;

const string = (value: unknown): string =>
  typeof value === 'string' ? value : '';

const unitInterval = (value: unknown): boolean => {
  const candidate = number(value);

  return candidate !== undefined && candidate >= 0 && candidate <= 1;
};

const positive = (value: unknown): boolean => (number(value) ?? 0) > 0;

const approximatelyEqual = (left: number, right: number): boolean =>
  Math.abs(left - right) <= 1e-12;

const rounded = (value: number): number => Number(value.toFixed(12));

const nonnegativeInteger = (value: unknown): boolean =>
  Number.isSafeInteger(value) && (value as number) >= 0;

const positiveInteger = (value: unknown): boolean =>
  Number.isSafeInteger(value) && (value as number) > 0;

const isCampaignAction = (value: unknown): value is 'smoke' | 'pilot' | 'run' =>
  value === 'smoke' || value === 'pilot' || value === 'run';

const sha256Digest = (value: unknown): value is string =>
  typeof value === 'string' && /^sha256:[a-f0-9]{64}$/i.test(value);

const hasNull = (value: Json, key: string): boolean =>
  Object.hasOwn(value, key) && value[key] === null;

const sortedKeys = (value: Json): readonly string[] =>
  Object.keys(value).sort();

const sorted = (values: readonly string[]): readonly string[] =>
  [...values].sort();

const stringArray = (value: unknown): readonly string[] | undefined =>
  Array.isArray(value) && value.every((item) => typeof item === 'string')
    ? value
    : undefined;

const arraysEqual = (
  left: readonly string[],
  right: readonly string[],
): boolean => JSON.stringify(left) === JSON.stringify(right);
