import { spawn } from 'node:child_process';
import { constants } from 'node:fs';
import {
  access,
  mkdir,
  open,
  readFile,
  rename,
  stat,
  writeFile,
} from 'node:fs/promises';
import { dirname, isAbsolute, join, parse, relative, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

export const benchmarkRoot = resolve(
  dirname(fileURLToPath(import.meta.url)),
  '..',
);
export const repoRoot = resolve(benchmarkRoot, '../..');
const cliPath = join(benchmarkRoot, 'dist/src/cli.js');
const apiPath = join(benchmarkRoot, 'dist/src/index.js');

export const template = {
  schemaVersion: 1,
  studyId: 'mosaic-study-001',
  root: '/absolute/path/to/mosaic-study-001',
  analysisImage: 'registry.example/mosaic-analysis@sha256:REPLACE_WITH_DIGEST',
  frozenAt: '2026-08-08T00:00:00.000Z',
  candidate: {
    provider: 'openrouter',
    model: 'qwen/qwen3.7-flash',
    effort: 'medium',
  },
  files: {
    prices: '/absolute/path/to/prices.json',
    calibrationCases: '/absolute/path/to/calibration-cases.json',
    confirmatoryCases: '/absolute/path/to/confirmatory-cases.json',
    powerConfig: '/absolute/path/to/power-config.json',
  },
  seeds: {
    pilot: 'pilot-seed-001',
    calibrationSchedule: 'calibration-schedule-seed-001',
    calibrationBootstrap: 'calibration-bootstrap-seed-001',
    confirmatorySchedule: 'confirmatory-schedule-seed-001',
  },
};

export const usage = `Usage:
  node benchmarks/mosaic/scripts/study.mjs --print-config
  node benchmarks/mosaic/scripts/study.mjs --validate-config <study.json>
  node benchmarks/mosaic/scripts/study.mjs --config <study.json> --yes-paid-study

The analysis image must already exist by immutable digest. Calibration and
confirmatory cases, prices, and the power config are human-authored inputs.
The primary model is frozen as openai/gpt-5.6-luna at medium effort and is not
configured here. candidate is the required non-OpenAI replication model.
The command runs all deterministic gates, paid schedules, scoring, freeze,
replication, sensitivity, and R analyses. It is resumable and never deletes
an existing attempt. Blinded human review and publication packaging remain
explicit post-study steps.`;

export const fail = (message) => {
  throw new Error(message);
};

const object = (value, name) => {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    fail(`${name} must be an object`);
  }
  return value;
};

const string = (value, name) => {
  if (typeof value !== 'string' || value.trim().length === 0) {
    fail(`${name} must be a non-empty string`);
  }
  return value;
};

const exactKeys = (value, expected, name) => {
  const actual = Object.keys(value).sort();
  if (actual.join('\0') !== [...expected].sort().join('\0')) {
    fail(`${name} must contain exactly: ${expected.join(', ')}`);
  }
};

export const exists = async (path) =>
  access(path, constants.F_OK).then(
    () => true,
    () => false,
  );

export const parseArgs = (argv) => {
  const result = {
    config: undefined,
    validate: undefined,
    paid: false,
    help: false,
    print: false,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === '--yes-paid-study') {
      if (result.paid) fail('duplicate argument: --yes-paid-study');
      result.paid = true;
    } else if (arg === '--help' || arg === '-h') {
      if (result.help) fail(`duplicate argument: ${arg}`);
      result.help = true;
    } else if (arg === '--print-config') {
      if (result.print) fail('duplicate argument: --print-config');
      result.print = true;
    } else if (arg === '--config' || arg === '--validate-config') {
      const value = argv[(index += 1)];
      if (value === undefined || value.startsWith('--')) {
        fail(`${arg} requires a path`);
      }
      if (arg === '--config') {
        if (result.config !== undefined) fail('duplicate argument: --config');
        result.config = value;
      } else {
        if (result.validate !== undefined) {
          fail('duplicate argument: --validate-config');
        }
        result.validate = value;
      }
    } else fail(`unknown argument: ${arg}`);
  }
  const modes = [
    result.help,
    result.print,
    result.validate !== undefined,
  ].filter(Boolean).length;
  if (
    modes > 1 ||
    (modes > 0 && (result.config !== undefined || result.paid))
  ) {
    fail(
      'help, print, validation, and paid execution modes cannot be combined',
    );
  }
  return result;
};

const fields = (value) => {
  const input = object(value, 'config');
  exactKeys(
    input,
    [
      'schemaVersion',
      'studyId',
      'root',
      'analysisImage',
      'frozenAt',
      'candidate',
      'files',
      'seeds',
    ],
    'config',
  );
  if (input.schemaVersion !== 1) fail('config.schemaVersion must equal 1');
  const files = object(input.files, 'config.files');
  const seeds = object(input.seeds, 'config.seeds');
  const candidate = object(input.candidate, 'config.candidate');
  exactKeys(
    files,
    ['prices', 'calibrationCases', 'confirmatoryCases', 'powerConfig'],
    'config.files',
  );
  exactKeys(
    seeds,
    [
      'pilot',
      'calibrationSchedule',
      'calibrationBootstrap',
      'confirmatorySchedule',
    ],
    'config.seeds',
  );
  exactKeys(candidate, ['provider', 'model', 'effort'], 'config.candidate');
  return { input, files, seeds, candidate };
};

const materializeFields = ({ input, files, seeds, candidate }) => ({
  schemaVersion: 1,
  studyId: string(input.studyId, 'config.studyId'),
  root: string(input.root, 'config.root'),
  analysisImage: string(input.analysisImage, 'config.analysisImage'),
  frozenAt: string(input.frozenAt, 'config.frozenAt'),
  candidate: {
    provider: string(candidate.provider, 'config.candidate.provider'),
    model: string(candidate.model, 'config.candidate.model'),
    effort: string(candidate.effort, 'config.candidate.effort'),
  },
  files: Object.fromEntries(
    ['prices', 'calibrationCases', 'confirmatoryCases', 'powerConfig'].map(
      (name) => [name, string(files[name], `config.files.${name}`)],
    ),
  ),
  seeds: Object.fromEntries(
    [
      'pilot',
      'calibrationSchedule',
      'calibrationBootstrap',
      'confirmatorySchedule',
    ].map((name) => [name, string(seeds[name], `config.seeds.${name}`)]),
  ),
});

const validateIdentity = (config) => {
  if (!/^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/u.test(config.studyId)) {
    fail('config.studyId is not a benchmark ID');
  }
  if (!isAbsolute(config.root)) fail('config.root must be absolute');
  const root = resolve(config.root);
  if (root === parse(root).root)
    fail('config.root cannot be a filesystem root');
  const rootRelation = relative(repoRoot, root);
  if (rootRelation === '' || !rootRelation.startsWith('..')) {
    fail('config.root must remain outside the repository');
  }
  if (!/^(?:[^\s]+@)?sha256:[a-f0-9]{64}$/u.test(config.analysisImage)) {
    fail('config.analysisImage must use an immutable SHA-256 digest');
  }
};

const validateProtocol = (config) => {
  if (
    config.candidate.effort !== 'medium' ||
    !/^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/u.test(config.candidate.provider) ||
    config.candidate.model !== config.candidate.model.trim() ||
    config.candidate.provider.toLowerCase() === 'openai' ||
    config.candidate.model.toLowerCase().startsWith('openai/')
  ) {
    fail('config.candidate must be a non-OpenAI model at medium effort');
  }
  const instant = new Date(config.frozenAt);
  if (
    !Number.isFinite(instant.valueOf()) ||
    instant.toISOString() !== config.frozenAt
  ) {
    fail('config.frozenAt must be a canonical ISO-8601 instant');
  }
};

/** Validates the complete trust-boundary config without starting a study. */
export const validateConfig = async (value) => {
  const config = materializeFields(fields(value));
  validateIdentity(config);
  validateProtocol(config);
  for (const path of Object.values(config.files)) {
    if (
      !isAbsolute(path) ||
      !(await exists(path)) ||
      !(await stat(path)).isFile()
    ) {
      fail(`study input is not an absolute regular file: ${path}`);
    }
  }
  return config;
};

export const pathsFor = (root) => ({
  inputs: join(root, 'inputs'),
  preflight: join(root, 'preflight'),
  pilot: join(root, 'pilot'),
  calibration: join(root, 'calibration'),
  power: join(root, 'power'),
  freeze: join(root, 'freeze'),
  schedules: join(root, 'schedules'),
  primary: join(root, 'primary'),
  replication: join(root, 'replication'),
  sensitivity: join(root, 'sensitivity'),
  analysisConfig: join(root, 'analysis', 'config'),
  final: join(root, 'final'),
});

const copyExact = async (source, destination) => {
  const bytes = await readFile(source);
  if (await exists(destination)) {
    if (!bytes.equals(await readFile(destination))) {
      fail(`immutable study input differs: ${destination}`);
    }
    return;
  }
  await mkdir(dirname(destination), { recursive: true });
  await writeFile(destination, bytes, { flag: 'wx', mode: 0o600 });
};

/** Copies every scientific input once so it cannot drift during long runs. */
export const materializeInputs = async (config, configPath, paths) => {
  const destinations = {
    prices: join(paths.inputs, 'prices.json'),
    calibrationCases: join(paths.inputs, 'calibration-cases.json'),
    confirmatoryCases: join(paths.inputs, 'confirmatory-cases.json'),
    powerConfig: join(paths.inputs, 'power-config.json'),
  };
  await copyExact(configPath, join(paths.inputs, 'study.json'));
  await Promise.all(
    Object.entries(config.files).map(([name, source]) =>
      copyExact(source, destinations[name]),
    ),
  );
  return { ...config, files: destinations };
};

/** Loads the built benchmark API only after the preflight build succeeds. */
export const loadApi = async () => import(pathToFileURL(apiPath).href);

export const execute = (program, args, options = {}) =>
  new Promise((resolvePromise, reject) => {
    const child = spawn(program, args, {
      cwd: options.cwd ?? repoRoot,
      env: process.env,
      stdio: options.stdio ?? ['ignore', process.stderr, process.stderr],
    });
    child.once('error', reject);
    child.once('close', (code) =>
      code === 0
        ? resolvePromise()
        : reject(new Error(`${options.label ?? program} exited with ${code}`)),
    );
  });

const output = async (program, args) => {
  const chunks = [];
  await new Promise((resolvePromise, reject) => {
    const child = spawn(program, args, {
      cwd: repoRoot,
      env: process.env,
      stdio: ['ignore', 'pipe', 'inherit'],
    });
    child.stdout.on('data', (chunk) => chunks.push(chunk));
    child.once('error', reject);
    child.once('close', (code) =>
      code === 0
        ? resolvePromise()
        : reject(new Error(`${program} exited with ${code}`)),
    );
  });
  return Buffer.concat(chunks).toString('utf8').trim();
};

const assertReceipt = async (path, expectedCommand) => {
  const value = JSON.parse(await readFile(path, 'utf8'));
  if (
    typeof value !== 'object' ||
    value === null ||
    Array.isArray(value) ||
    value.schemaVersion !== 1 ||
    value.ok !== true ||
    value.command !== expectedCommand
  ) {
    fail(`invalid benchmark receipt: ${path}`);
  }
  return value;
};

const stepState = async (step) => {
  const outputs = step.outputs ?? [];
  const receipt = await exists(step.receipt);
  const present = await Promise.all(outputs.map(exists));
  return {
    receipt,
    anyOutput: present.some(Boolean),
    allOutputs: present.every(Boolean),
  };
};

/** Runs one CLI stage with an immutable receipt and recoverable failed output. */
export const cli = async (step) => {
  const state = await stepState(step);
  if (state.receipt && state.allOutputs) {
    process.stderr.write(`study: ${step.label} already complete\n`);
    return assertReceipt(step.receipt, step.args[0]);
  }
  if (state.receipt) {
    fail(`${step.label} has a receipt but lacks an expected output`);
  }
  if (state.anyOutput) {
    fail(`${step.label} has an output but no immutable receipt`);
  }
  await mkdir(dirname(step.receipt), { recursive: true });
  const partial = `${step.receipt}.partial.${process.pid}.${Date.now()}`;
  const handle = await open(partial, 'wx', 0o600);
  let closed = false;
  process.stderr.write(`study: ${step.label}\n`);
  try {
    await execute(process.execPath, [cliPath, ...step.args], {
      label: step.label,
      stdio: ['ignore', handle.fd, 'inherit'],
    });
    await handle.close();
    closed = true;
    const receipt = await assertReceipt(partial, step.args[0]);
    const outputs = step.outputs ?? [];
    if (!(await Promise.all(outputs.map(exists))).every(Boolean)) {
      fail(`${step.label} finished without every expected output`);
    }
    await rename(partial, step.receipt);
    return receipt;
  } catch (error) {
    if (!closed) await handle.close();
    if (await exists(partial)) {
      await rename(
        partial,
        `${step.receipt}.failed.${process.pid}.${Date.now()}.json`,
      );
    }
    throw error;
  }
};

export const writeJson = async (context, path, value) => {
  const content = `${context.api.core.canonicalJson(value)}\n`;
  if (await exists(path)) {
    if ((await readFile(path, 'utf8')) !== content) {
      fail(`immutable generated input differs: ${path}`);
    }
    return;
  }
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, content, { flag: 'wx', mode: 0o600 });
};

export const readJson = async (path) =>
  JSON.parse(await readFile(path, 'utf8'));

export const cleanCommit = async () => {
  const status = await output('git', [
    'status',
    '--porcelain=v1',
    '--untracked-files=all',
  ]);
  if (status.length > 0) fail('official study requires a clean worktree');
  return output('git', ['rev-parse', 'HEAD']);
};
