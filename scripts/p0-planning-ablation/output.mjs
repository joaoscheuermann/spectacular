import { createHash, randomUUID } from 'node:crypto';
import { execFile } from 'node:child_process';
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { promisify } from 'node:util';

const execute = promisify(execFile);
const tokenFields = [
  'inputTokens',
  'outputTokens',
  'totalTokens',
  'reasoningTokens',
  'cachedInputTokens',
  'cacheWriteTokens',
  'searchUnits',
];

const files = async (directory, extension, prefix) =>
  (await readdir(directory, { withFileTypes: true }))
    .filter((entry) => entry.isFile() && entry.name.endsWith(extension))
    .map(({ name }) => ({
      name: `${prefix}/${name}`,
      path: join(directory, name),
    }))
    .sort((left, right) => left.name.localeCompare(right.name));

const digest = async (entries) => {
  const hash = createHash('sha256');
  for (const { name, path } of entries) {
    const data = await readFile(path);
    hash.update(name).update('\0').update(String(data.byteLength)).update('\0');
    hash.update(data);
  }
  return hash.digest('hex');
};

const fileDigest = async (path) =>
  createHash('sha256')
    .update(await readFile(path))
    .digest('hex');

const json = (path, value) =>
  writeFile(path, `${JSON.stringify(value, null, 2)}\n`, 'utf8');

const usageBucket = () => ({
  calls: 0,
  withUsage: 0,
  withCost: 0,
  totals: Object.fromEntries(tokenFields.map((field) => [field, 0])),
  costs: new Map(),
});

const addUsage = (bucket, usage) => {
  bucket.calls += 1;
  if (usage === undefined || usage === null) return;

  bucket.withUsage += 1;
  for (const field of tokenFields) bucket.totals[field] += usage[field] ?? 0;

  if (usage.cost === undefined || usage.cost === null) return;
  bucket.withCost += 1;
  const unit = usage.cost.unit ?? 'unspecified';
  const current = bucket.costs.get(unit) ?? {
    amount: 0,
    upstreamAmount: undefined,
  };
  bucket.costs.set(unit, {
    amount: current.amount + Number(usage.cost.amount ?? 0),
    upstreamAmount:
      current.upstreamAmount === undefined &&
      usage.cost.upstreamAmount === undefined
        ? undefined
        : (current.upstreamAmount ?? 0) +
          Number(usage.cost.upstreamAmount ?? 0),
  });
};

const snapshotBucket = (bucket) => ({
  calls: {
    total: bucket.calls,
    withUsage: bucket.withUsage,
    withCost: bucket.withCost,
  },
  ...bucket.totals,
  costs: [...bucket.costs.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([unit, { amount, upstreamAmount }]) => ({
      unit,
      amount,
      ...(upstreamAmount === undefined ? {} : { upstreamAmount }),
    })),
});

const createUsage = () => {
  let sequence = 0;
  let attemptSequence = 0;
  const total = usageBucket();
  const operations = new Map();
  const attempts = new Map();

  const recordAttempt = ({ operation, model }) => {
    attemptSequence += 1;
    attempts.set(operation, (attempts.get(operation) ?? 0) + 1);
    return { attempt: attemptSequence, operation, model };
  };

  const record = ({ operation, model, usage }) => {
    sequence += 1;
    const operationBucket = operations.get(operation) ?? usageBucket();
    operations.set(operation, operationBucket);
    addUsage(total, usage);
    addUsage(operationBucket, usage);

    return {
      sequence,
      operation,
      model,
      usage: usage ?? null,
    };
  };

  const snapshot = () => {
    const aggregate = snapshotBucket(total);
    return {
      ...aggregate,
      attempts: {
        total: attemptSequence,
        byOperation: Object.fromEntries(
          [...attempts.entries()].sort(([left], [right]) =>
            left.localeCompare(right),
          ),
        ),
      },
      calls: {
        ...aggregate.calls,
        byOperation: Object.fromEntries(
          [...operations.entries()]
            .sort(([left], [right]) => left.localeCompare(right))
            .map(([name, bucket]) => [name, bucket.calls]),
        ),
      },
      byOperation: Object.fromEntries(
        [...operations.entries()]
          .sort(([left], [right]) => left.localeCompare(right))
          .map(([name, bucket]) => [name, snapshotBucket(bucket)]),
      ),
    };
  };

  return { record, recordAttempt, snapshot };
};

const repositoryIdentity = async (root) => {
  const [commit, status] = await Promise.all([
    execute('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }),
    execute('git', ['status', '--porcelain'], {
      cwd: root,
      encoding: 'utf8',
    }),
  ]);
  return {
    commit: commit.stdout.trim(),
    dirty: status.stdout.trim().length > 0,
  };
};

const sourceEntries = (directory) =>
  [
    'comparison.mjs',
    'index.mjs',
    'inputs.mjs',
    'output.mjs',
    'prompt.mjs',
    'rejudge-inputs.mjs',
    'rejudge.mjs',
    'runtime.mjs',
    'schemas.mjs',
    'utils.mjs',
  ].map((name) => ({
    name: `scripts/p0-planning-ablation/${name}`,
    path: join(directory, name),
  }));

/** Reads the local immutable inputs without creating a run directory. */
export const readInputIdentity = async (directory) => {
  const [caseEntries, catalogEntries, fixtureSha256] = await Promise.all([
    files(join(directory, 'cases'), '.json', 'cases'),
    files(join(directory, 'cases', 'skills'), '.md', 'cases/skills'),
    fileDigest(join(directory, 'fixtures', 'p0.json')),
  ]);
  const [casesSha256, catalogSha256] = await Promise.all([
    digest(caseEntries),
    digest(catalogEntries),
  ]);

  return {
    fixture: { sha256: fixtureSha256 },
    cases: { count: caseEntries.length, sha256: casesSha256 },
    catalog: { count: catalogEntries.length, sha256: catalogSha256 },
  };
};

/** Creates one ignored, identity-bound output directory for a paid run. */
export const createOutput = async ({
  directory,
  config,
  mode,
  lineage,
  fixtureSource,
  fixtureCases,
  validatedInputIdentity,
}) => {
  const id = randomUUID();
  const startedAt = new Date().toISOString();
  const root = resolve(directory, '..', '..');
  const runDirectory = join(directory, 'output', id);
  const manifestPath = join(runDirectory, 'manifest.json');
  const resultsPath = join(runDirectory, 'results.json');
  const logPath = join(runDirectory, 'output.log');
  const usage = createUsage();
  await mkdir(runDirectory, { recursive: true });

  const [localInputs, repository] = await Promise.all([
    readInputIdentity(directory),
    repositoryIdentity(root),
  ]);
  if (
    validatedInputIdentity !== undefined &&
    JSON.stringify(localInputs) !== JSON.stringify(validatedInputIdentity)
  ) {
    throw new Error('Local inputs changed after validation.');
  }
  const sources = sourceEntries(directory);
  const packageMetadata = [
    { name: 'package.json', path: join(root, 'package.json') },
    { name: 'package-lock.json', path: join(root, 'package-lock.json') },
  ];
  const [sourcesSha256, packageSha256] = await Promise.all([
    digest(sources),
    digest(packageMetadata),
  ]);
  const identity = {
    repository,
    fixture: {
      cases: fixtureCases,
      runId: fixtureSource.runId,
      sourceResultsSha256: fixtureSource.resultsSha256,
      sha256: localInputs.fixture.sha256,
    },
    cases: localInputs.cases,
    catalog: localInputs.catalog,
    experimentSources: { count: sources.length, sha256: sourcesSha256 },
    packageMetadata: {
      count: packageMetadata.length,
      sha256: packageSha256,
    },
    environment: {
      node: process.version,
      platform: process.platform,
      arch: process.arch,
    },
  };
  const manifest = {
    schemaVersion: 1,
    runId: id,
    mode,
    status: 'running',
    startedAt,
    config,
    ...(lineage === undefined ? {} : { lineage }),
    identity,
    files: { log: 'output.log', results: 'results.json' },
  };
  await json(manifestPath, manifest);

  return {
    id,
    identity,
    logPath,
    recordProviderAttempt: usage.recordAttempt,
    recordProviderUsage: usage.record,
    providerUsage: usage.snapshot,
    async complete(results, metrics) {
      const completedAt = new Date().toISOString();
      const providerUsage = usage.snapshot();
      await json(resultsPath, {
        runId: id,
        mode,
        ...(lineage === undefined ? {} : { lineage }),
        cases: results.length,
        metrics,
        providerUsage,
        results,
      });
      await json(manifestPath, {
        ...manifest,
        status: 'completed',
        completedAt,
        cases: results.length,
        metrics,
        providerUsage,
      });
    },
    fail: () =>
      json(manifestPath, {
        ...manifest,
        status: 'failed',
        failedAt: new Date().toISOString(),
        providerUsage: usage.snapshot(),
      }),
  };
};
