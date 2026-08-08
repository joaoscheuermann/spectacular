import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { access, mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { promisify } from 'node:util';
import test from 'node:test';

const executeFile = promisify(execFile);
const script = 'benchmarks/mosaic/scripts/study.mjs';

test('prints a complete study config template without side effects', async () => {
  const result = await executeFile(process.execPath, [
    script,
    '--print-config',
  ]);
  const config = JSON.parse(result.stdout) as Readonly<Record<string, unknown>>;

  assert.equal(config['schemaVersion'], 1);
  assert.equal(config['studyId'], 'mosaic-study-001');
  assert.deepEqual(Object.keys(config['files'] as object).sort(), [
    'calibrationCases',
    'confirmatoryCases',
    'powerConfig',
    'prices',
  ]);
  assert.deepEqual(Object.keys(config['seeds'] as object).sort(), [
    'calibrationBootstrap',
    'calibrationSchedule',
    'confirmatorySchedule',
    'pilot',
  ]);
  assert.equal(result.stderr, '');
});

test('requires explicit acknowledgment before a paid study', async () => {
  await assert.rejects(
    executeFile(process.execPath, [script, '--config', '/missing/config.json']),
    (error: unknown) => {
      assert.equal(typeof error, 'object');
      assert.notEqual(error, null);
      const result = error as {
        readonly code?: number;
        readonly stderr?: string;
      };
      assert.equal(result.code, 1);
      assert.match(result.stderr ?? '', /--yes-paid-study/u);
      return true;
    },
  );
});

test('validates an exact study config without creating its study root', async (context) => {
  const temporary = await mkdtemp(join(tmpdir(), 'mosaic-study-config-'));
  context.after(() => rm(temporary, { recursive: true, force: true }));
  const authored = join(temporary, 'authored');
  const root = join(temporary, 'output');
  await mkdir(authored);
  const files = {
    prices: join(authored, 'prices.json'),
    calibrationCases: join(authored, 'calibration-cases.json'),
    confirmatoryCases: join(authored, 'confirmatory-cases.json'),
    powerConfig: join(authored, 'power-config.json'),
  };
  await Promise.all(
    Object.values(files).map((path) => writeFile(path, '{}\n')),
  );
  const configPath = join(authored, 'study.json');
  const config = {
    schemaVersion: 1,
    studyId: 'study-config-test',
    root,
    analysisImage: `sha256:${'a'.repeat(64)}`,
    frozenAt: '2026-08-08T00:00:00.000Z',
    candidate: {
      provider: 'openrouter',
      model: 'qwen/qwen3.7-flash',
      effort: 'medium',
    },
    files,
    seeds: {
      pilot: 'pilot',
      calibrationSchedule: 'calibration-schedule',
      calibrationBootstrap: 'calibration-bootstrap',
      confirmatorySchedule: 'confirmatory-schedule',
    },
  };
  await writeFile(configPath, `${JSON.stringify(config)}\n`);

  const result = await executeFile(process.execPath, [
    script,
    '--validate-config',
    configPath,
  ]);
  const output = JSON.parse(result.stdout) as Readonly<Record<string, unknown>>;
  assert.equal(output['ok'], true);
  assert.equal(output['studyId'], 'study-config-test');
  await assert.rejects(
    access(root),
    (error: unknown) => (error as NodeJS.ErrnoException).code === 'ENOENT',
  );

  const invalidPath = join(authored, 'study.invalid.json');
  await writeFile(
    invalidPath,
    `${JSON.stringify({ ...config, unexpected: true })}\n`,
  );
  await assert.rejects(
    executeFile(process.execPath, [script, '--validate-config', invalidPath]),
    (error: unknown) => {
      const result = error as { readonly stdout?: string };
      assert.match(result.stdout ?? '', /must contain exactly/u);
      return true;
    },
  );
});
