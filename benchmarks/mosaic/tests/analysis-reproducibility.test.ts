import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

import {
  AnalysisResultV1,
  ContrastV1,
  PowerResultV1,
} from '../src/schemas/index.js';
import { holmAdjust, modelCallBudgetAtP95 } from '../src/scoring/index.js';
import type { ScoreRow } from '../src/schemas/index.js';

const readJson = async (path: string): Promise<unknown> =>
  JSON.parse(await readFile(path, 'utf8')) as unknown;

test('golden power rules satisfy the frozen schema and balancing arithmetic', async () => {
  const parsed = PowerResultV1.parse(
    await readJson('benchmarks/mosaic/analysis/goldens/power.json'),
  );

  assert.equal(parsed.nPower % 24, 0);
  assert.equal(parsed.nFinal % 120, 0);
  assert.equal(parsed.numericSeed, 104729);
  assert.equal(parsed.baselineProbability, 0.45);
  assert.equal(parsed.randomInterceptSd, 0.7);
  assert.deepEqual(parsed.adaptiveClasses, ['E', 'F']);
  const config = await readFile(
    'benchmarks/mosaic/analysis/fixtures/power-config.json',
  );
  const analysisConfig = (await readJson(
    'benchmarks/mosaic/analysis/fixtures/analysis-config.json',
  )) as {
    readonly freeze: {
      readonly artifactHashes: {
        readonly powerConfig: string;
        readonly powerResult: string;
      };
    };
  };
  const powerResult = await readFile(
    'benchmarks/mosaic/analysis/fixtures/power-result.json',
  );
  assert.equal(
    parsed.configHash,
    `sha256:${createHash('sha256').update(config).digest('hex')}`,
  );
  assert.equal(
    parsed.configHash,
    analysisConfig.freeze.artifactHashes.powerConfig,
  );
  assert.equal(
    analysisConfig.freeze.artifactHashes.powerResult,
    `sha256:${createHash('sha256').update(powerResult).digest('hex')}`,
  );
});

test('power simulation and report consume every registered provenance parameter', async () => {
  const [powerSource, reportSource] = await Promise.all([
    readFile('benchmarks/mosaic/analysis/power.R', 'utf8'),
    readFile('benchmarks/mosaic/analysis/report.R', 'utf8'),
  ]);

  assert.match(powerSource, /configHash = sha256_file\(args\[\[1\]\]\)/);
  assert.match(powerSource, /classes = as\.list\(adaptive_classes\)/);
  for (const field of [
    'numericSeed',
    'configHash',
    'baselineProbability',
    'randomInterceptSd',
    'adaptiveClasses',
  ]) {
    assert.match(powerSource, new RegExp(field));
    assert.match(reportSource, new RegExp(field));
  }
});

test('all analysis families validate power while only primary families materialize it', async () => {
  const [analysisSource, validationSource] = await Promise.all([
    readFile('benchmarks/mosaic/analysis/analyze.R', 'utf8'),
    readFile('benchmarks/mosaic/analysis/validate.R', 'utf8'),
  ]);

  assert.match(
    analysisSource,
    /validated_power <- validate_power_result_file\(power_path, config\)/,
  );
  assert.match(
    analysisSource,
    /power <- if \(power_family\) validated_power else NULL/,
  );
  assert.match(validationSource, /bootstrapRepetitions must equal 10000/);
  assert.match(
    validationSource,
    /bootstrapSeed must equal powerParameters\.numericSeed/,
  );
  assert.match(validationSource, /family != "secondary"/);
});

test('Holm golden preserves the registered step-down values', async () => {
  const golden = (await readJson(
    'benchmarks/mosaic/analysis/goldens/holm.json',
  )) as {
    readonly input: readonly number[];
    readonly expected: readonly number[];
  };
  const adjusted = holmAdjust(
    golden.input.map((p, index) => ({ id: `h${index + 1}`, p })),
  );

  assert.deepEqual(
    adjusted.map(({ adjusted: value }) => value),
    golden.expected,
  );
});

test('fallback golden matches the implemented estimator order and fit-rate floor', async () => {
  const golden = (await readJson(
    'benchmarks/mosaic/analysis/goldens/fallbacks.json',
  )) as {
    readonly order: readonly string[];
    readonly minimumValidFitRate: number;
  };
  const source = await readFile('benchmarks/mosaic/analysis/fit.R', 'utf8');

  assert.match(
    source,
    /order <- c\("bobyqa", "nloptwrap", "hc2", "bootstrap"\)/,
  );
  assert.deepEqual(golden.order.at(-1), 'not-estimable');
  assert.equal(golden.minimumValidFitRate, 0.95);
});

test('contrast schema reports separate odds-ratio and absolute-difference intervals', () => {
  const parsed = ContrastV1.parse({
    id: 'M1-vs-B2-primary',
    estimate: 0.4,
    oddsRatio: Math.exp(0.4),
    oddsRatioConfidence95: { lower: 1.1, upper: 2.0 },
    absoluteDifference: 0.1,
    absoluteDifferenceConfidence95: { lower: 0.02, upper: 0.18 },
    pValue: 0.02,
    holmPValue: 0.04,
  });

  assert.equal(parsed.oddsRatioConfidence95.lower, 1.1);
  assert.equal(parsed.absoluteDifferenceConfidence95.upper, 0.18);
});

test('analysis result preserves the exact formula and not-estimable fallback', () => {
  const hash = `sha256:${'a'.repeat(64)}`;
  const parsed = AnalysisResultV1.parse({
    schemaVersion: 1,
    studyId: 'study-1',
    family: 'primary',
    datasetHash: hash,
    freezeHash: hash,
    implementationHash: hash,
    formula: 'success ~ condition * composition_class + (1 | case_id)',
    estimator: 'not-estimable',
    estimable: false,
    validFitRate: 0.949,
    observations: 240,
    cases: 24,
    contrasts: [],
    power: null,
    warnings: ['bootstrap_fit_rate_below_95pct'],
    resultHash: hash,
  });

  assert.equal(parsed.estimable, false);
});

test('budget sensitivity uses the nearest-rank baseline pilot p95 including failures', () => {
  const rows = Array.from({ length: 20 }, (_, index) => ({
    phase: 'pilot',
    conditionId: 'B2',
    modelCalls: index + 1,
  })) as ScoreRow[];

  assert.equal(modelCallBudgetAtP95(rows, 'B2'), 19);
});

test('container verifier disables network and requires two byte-identical runs', async () => {
  const dockerfile = await readFile(
    'benchmarks/mosaic/container/Dockerfile',
    'utf8',
  );
  const script = await readFile(
    'benchmarks/mosaic/container/verify-determinism.sh',
    'utf8',
  );

  assert.match(dockerfile, /R_LIBS=\/opt\/renv\/library/);
  assert.match(dockerfile, /apt-get install -y --no-install-recommends cmake/);
  assert.match(dockerfile, /mkdir -p \/opt\/renv\/library/);
  assert.match(dockerfile, /Archive\/renv\/renv_1\.1\.5\.tar\.gz/);
  assert.match(dockerfile, /packageVersion\(package\)/);
  assert.match(script, /--network none/);
  assert.match(script, /--pull never/);
  assert.match(script, /--tmpfs \/tmp:rw,noexec,nosuid,size=256m/);
  assert.match(
    script,
    /run_analysis result-1\.json\nrun_analysis result-2\.json/,
  );
  assert.match(script, /cmp .*result-1\.json.*result-2\.json/);
  assert.match(script, /run_report result-1\.json report-1\.md/);
  assert.match(script, /run_report result-2\.json report-2\.md/);
  assert.match(script, /cmp .*report-1\.md.*report-2\.md/);
  assert.match(script, /report-1\.md.*report-2\.md.*SHA256SUMS/s);
});

test('power verifier uses the same immutable offline two-run gate', async () => {
  const script = await readFile(
    'benchmarks/mosaic/container/verify-power.sh',
    'utf8',
  );

  assert.match(script, /@sha256:/);
  assert.match(script, /--network none/);
  assert.match(script, /--pull never/);
  assert.match(script, /--read-only/);
  assert.match(script, /--tmpfs \/tmp:rw,noexec,nosuid,size=256m/);
  assert.match(script, /run_once power-1\.json\nrun_once power-2\.json/);
  assert.match(script, /cmp .*power-1\.json.*power-2\.json/);
  assert.match(script, /power-1\.json.*power-2\.json.*SHA256SUMS/s);
});
