import { createHash, randomUUID } from 'node:crypto';
import { execFile } from 'node:child_process';
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { promisify } from 'node:util';

const execute = promisify(execFile);

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

const json = (path, value) =>
  writeFile(path, `${JSON.stringify(value, null, 2)}\n`, 'utf8');

const ratio = (numerator, denominator) =>
  denominator === 0 ? 0 : numerator / denominator;

const harmonicMean = (left, right) => ratio(2 * left * right, left + right);

const rates = ({
  selectedExpected,
  selectedUseful,
  selectedNoise,
  expected,
  selected,
  recoveredRelevant,
  recoveredNoise,
  removedNoise,
}) => {
  const recall = ratio(selectedExpected, expected);
  const precision = ratio(selectedExpected + selectedUseful, selected);
  const relevantRetentionRate = ratio(
    selectedExpected + selectedUseful,
    recoveredRelevant,
  );

  return {
    recall,
    noiseRate: ratio(selectedNoise, selected),
    precision,
    f1: harmonicMean(precision, recall),
    noiseRemovalRate: ratio(removedNoise, recoveredNoise),
    relevantRetentionRate,
    selectionF1: harmonicMean(precision, relevantRetentionRate),
  };
};

export const scoreBundle = (current, recovered, selected) => {
  const recoveredNames = new Set(recovered.map(({ name }) => name));
  const selectedNames = new Set(selected.map(({ name }) => name));
  const selectedExpected = current.skills.expected.filter((name) =>
    selectedNames.has(name),
  );
  const selectedUseful = current.skills.useful.filter((name) =>
    selectedNames.has(name),
  );
  const selectedNoise = Object.entries(current.skills.noise)
    .filter(([name]) => selectedNames.has(name))
    .map(([name, { type }]) => ({ name, type }));
  const recoveredRelevant = [
    ...current.skills.expected,
    ...current.skills.useful,
  ].filter((name) => recoveredNames.has(name));
  const recoveredNoise = Object.entries(current.skills.noise)
    .filter(([name]) => recoveredNames.has(name))
    .map(([name, { type }]) => ({ name, type }));
  const removedNoise = recoveredNoise.filter(
    ({ name }) => !selectedNames.has(name),
  );
  const counts = {
    selectedExpected: selectedExpected.length,
    selectedUseful: selectedUseful.length,
    selectedNoise: selectedNoise.length,
    expected: current.skills.expected.length,
    selected: selected.length,
    recoveredRelevant: recoveredRelevant.length,
    recoveredNoise: recoveredNoise.length,
    removedNoise: removedNoise.length,
    recovered: recovered.length,
  };

  return {
    counts,
    ...rates(counts),
    missing: current.skills.expected.filter((name) => !selectedNames.has(name)),
    selectedUseful,
    selectedNoise,
    removedNoise,
  };
};

export const aggregateMetrics = (results) => {
  const counts = results.reduce(
    (total, { metrics }) => ({
      selectedExpected:
        total.selectedExpected + metrics.counts.selectedExpected,
      selectedUseful: total.selectedUseful + metrics.counts.selectedUseful,
      selectedNoise: total.selectedNoise + metrics.counts.selectedNoise,
      expected: total.expected + metrics.counts.expected,
      selected: total.selected + metrics.counts.selected,
      recoveredRelevant:
        total.recoveredRelevant + metrics.counts.recoveredRelevant,
      recoveredNoise: total.recoveredNoise + metrics.counts.recoveredNoise,
      removedNoise: total.removedNoise + metrics.counts.removedNoise,
      recovered: total.recovered + metrics.counts.recovered,
    }),
    {
      selectedExpected: 0,
      selectedUseful: 0,
      selectedNoise: 0,
      expected: 0,
      selected: 0,
      recoveredRelevant: 0,
      recoveredNoise: 0,
      removedNoise: 0,
      recovered: 0,
    },
  );

  return { counts, ...rates(counts) };
};

const createUsage = () => {
  let sequence = 0;
  let callsWithUsage = 0;
  let callsWithCost = 0;
  const operations = new Map();
  const costs = new Map();
  const totals = {
    inputTokens: 0,
    outputTokens: 0,
    totalTokens: 0,
    reasoningTokens: 0,
    cachedInputTokens: 0,
    cacheWriteTokens: 0,
    searchUnits: 0,
  };

  const record = ({ operation, model, usage }) => {
    sequence += 1;
    operations.set(operation, (operations.get(operation) ?? 0) + 1);

    if (usage !== undefined) {
      callsWithUsage += 1;
      for (const name of Object.keys(totals)) {
        totals[name] += usage[name] ?? 0;
      }
      if (usage.cost !== undefined) {
        callsWithCost += 1;
        const unit = usage.cost.unit ?? 'unspecified';
        const current = costs.get(unit) ?? {
          amount: 0,
          upstreamAmount: undefined,
        };
        costs.set(unit, {
          amount: current.amount + usage.cost.amount,
          upstreamAmount:
            current.upstreamAmount === undefined &&
            usage.cost.upstreamAmount === undefined
              ? undefined
              : (current.upstreamAmount ?? 0) +
                (usage.cost.upstreamAmount ?? 0),
        });
      }
    }

    return {
      sequence,
      operation,
      model,
      usage: usage ?? null,
    };
  };

  const snapshot = () => ({
    calls: {
      total: sequence,
      withUsage: callsWithUsage,
      withCost: callsWithCost,
      byOperation: Object.fromEntries(
        [...operations.entries()].sort(([left], [right]) =>
          left.localeCompare(right),
        ),
      ),
    },
    ...totals,
    costs: [...costs.entries()]
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([unit, { amount, upstreamAmount }]) => ({
        unit,
        amount,
        ...(upstreamAmount === undefined ? {} : { upstreamAmount }),
      })),
  });

  return { record, snapshot };
};

/** Creates one ignored, identity-bound output directory for a paid run. */
export const createOutput = async ({ directory, config }) => {
  const id = randomUUID();
  const startedAt = new Date().toISOString();
  const root = resolve(directory, '..', '..');
  const runDirectory = join(directory, 'output', id);
  const manifestPath = join(runDirectory, 'manifest.json');
  const resultsPath = join(runDirectory, 'results.json');
  const logPath = join(runDirectory, 'output.log');
  const usage = createUsage();
  await mkdir(runDirectory, { recursive: true });

  const [cases, skills, commit, status] = await Promise.all([
    files(join(directory, 'cases'), '.json', 'cases'),
    files(join(directory, 'cases', 'skills'), '.md', 'cases/skills'),
    execute('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }),
    execute('git', ['status', '--porcelain'], {
      cwd: root,
      encoding: 'utf8',
    }),
  ]);
  const sources = [
    ...[
      'index.mjs',
      'output.mjs',
      'prompt.mjs',
      'schemas.mjs',
      'utils.mjs',
    ].map((name) => ({
      name: `scripts/direct-skill-planning-gated/${name}`,
      path: join(directory, name),
    })),
    { name: 'package.json', path: join(root, 'package.json') },
    { name: 'package-lock.json', path: join(root, 'package-lock.json') },
    ...cases,
    ...skills,
  ].sort((left, right) => left.name.localeCompare(right.name));
  const [sourceSha256, casesSha256, catalogSha256] = await Promise.all([
    digest(sources),
    digest(cases),
    digest(skills),
  ]);
  const identity = {
    source: {
      commit: commit.stdout.trim(),
      dirty: status.stdout.trim().length > 0,
      sha256: sourceSha256,
    },
    inputs: {
      cases: { count: cases.length, sha256: casesSha256 },
      catalog: { count: skills.length, sha256: catalogSha256 },
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
    status: 'running',
    startedAt,
    config,
    identity,
    files: { log: 'output.log', results: 'results.json' },
  };
  await json(manifestPath, manifest);

  return {
    id,
    identity,
    logPath,
    recordProviderUsage: usage.record,
    providerUsage: usage.snapshot,
    async complete(results, metrics) {
      const completedAt = new Date().toISOString();
      const providerUsage = usage.snapshot();
      await json(resultsPath, {
        runId: id,
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
