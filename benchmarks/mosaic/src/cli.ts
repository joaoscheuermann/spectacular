import { createHash } from 'node:crypto';
import { readFile, stat, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

import {
  campaign,
  type Benchmark,
  type CampaignAction,
  type CampaignCheck,
  type CampaignOptions,
  type CampaignRun,
} from './campaign.js';
import { compare, type CompareOptions, type CompareReport } from './compare.js';
import { serveAcp, type AcpOptions } from './acp.js';
import type { RunMode } from './run.js';

type Output = Pick<NodeJS.WriteStream, 'write'>;

export interface CliDependencies {
  readonly cwd?: string;
  readonly stdout?: Output;
  readonly stderr?: Output;
  readonly serveAcp?: (options: AcpOptions) => Promise<void>;
  readonly campaign?: (
    options: CampaignOptions,
  ) => Promise<CampaignCheck | CampaignRun>;
  readonly compare?: (options: CompareOptions) => Promise<CompareReport>;
}

/** Executes one benchmark command and returns its process exit code. */
export const runCli = async (
  args: readonly string[],
  dependencies: CliDependencies = {},
): Promise<number> => {
  const stdout = dependencies.stdout ?? process.stdout;
  const stderr = dependencies.stderr ?? process.stderr;

  try {
    switch (args[0]) {
      case 'serve':
        return await serve(args.slice(1), dependencies);
      case 'campaign':
        return await runCampaign(args.slice(1), dependencies, stdout);
      case 'compare':
        return await runComparison(args.slice(1), dependencies, stdout);
      case 'release':
        return await createRelease(args.slice(1), dependencies, stdout);
      case 'help':
      case '--help':
      case '-h':
      case undefined:
        stdout.write(`${usage}\n`);
        return args[0] === undefined ? 2 : 0;
      default:
        throw new Error(`Unknown command: ${args[0]}`);
    }
  } catch (error) {
    stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    return 2;
  }
};

const serve = async (
  args: readonly string[],
  dependencies: CliDependencies,
): Promise<number> => {
  const mode = parseMode(args[0]);
  rejectExtra(args, 1);
  await (dependencies.serveAcp ?? serveAcp)({ mode });
  return 0;
};

const runCampaign = async (
  args: readonly string[],
  dependencies: CliDependencies,
  stdout: Output,
): Promise<number> => {
  const benchmark = parseBenchmark(args[0]);
  const action = parseAction(args[1]);
  const flags = args.slice(2);
  const yesPaidRun = takeBooleanFlag(flags, '--yes-paid-run');
  const skillsbenchReport = takeValueFlag(flags, '--skillsbench-report');
  const rootDir =
    takeValueFlag(flags, '--root') ??
    (await benchmarkRoot(dependencies.cwd ?? process.cwd()));
  rejectFlags(flags);

  const result = await (dependencies.campaign ?? campaign)({
    benchmark,
    action,
    rootDir,
    yesPaidRun,
    ...(skillsbenchReport === undefined ? {} : { skillsbenchReport }),
  });
  writeJson(stdout, result);

  if ('ok' in result) return result.ok ? 0 : 1;
  return result.arms.every((arm) => arm.result.code === 0) ? 0 : 1;
};

const runComparison = async (
  args: readonly string[],
  dependencies: CliDependencies,
  stdout: Output,
): Promise<number> => {
  const flags = [...args];
  const directDir = takeRequiredValueFlag(flags, '--direct');
  const mosaicDir = takeRequiredValueFlag(flags, '--mosaic');
  const reportPath = takeValueFlag(flags, '--report');
  rejectFlags(flags);
  const result = await (dependencies.compare ?? compare)({
    directDir,
    mosaicDir,
    ...(reportPath === undefined ? {} : { reportPath }),
  });
  writeJson(stdout, result);
  return result.exitCode;
};

const createRelease = async (
  args: readonly string[],
  dependencies: CliDependencies,
  stdout: Output,
): Promise<number> => {
  rejectExtra(args, 0);
  const root = await benchmarkRoot(dependencies.cwd ?? process.cwd());
  const asset = resolve(root, 'dist', 'mosaic-bench-acp.mjs');
  const digest = createHash('sha256')
    .update(await readFile(asset))
    .digest('hex');
  const checksum = `${asset}.sha256`;
  await writeFile(checksum, `${digest}  mosaic-bench-acp.mjs\n`, 'utf8');
  writeJson(stdout, {
    version: '0.1.3',
    asset,
    checksum,
    sha256: digest,
  });
  return 0;
};

const parseMode = (value: string | undefined): RunMode => {
  if (value === 'direct' || value === 'mosaic') return value;
  throw new Error('serve requires one mode: direct or mosaic');
};

const parseBenchmark = (value: string | undefined): Benchmark => {
  if (value === 'skillsbench' || value === 'terminalbench') return value;
  throw new Error(
    'campaign requires one benchmark: skillsbench or terminalbench',
  );
};

const parseAction = (value: string | undefined): CampaignAction => {
  if (value === 'check' || value === 'smoke' || value === 'run') return value;
  throw new Error('campaign requires one action: check, smoke, or run');
};

const benchmarkRoot = async (cwd: string): Promise<string> => {
  const candidates = [resolve(cwd, 'benchmarks', 'mosaic'), resolve(cwd)];
  for (const candidate of candidates) {
    try {
      if ((await stat(resolve(candidate, 'project.json'))).isFile())
        return candidate;
    } catch {
      // Try the next supported invocation directory.
    }
  }
  throw new Error('Cannot find the mosaic-benchmark project root.');
};

const takeBooleanFlag = (args: string[], flag: string): boolean => {
  const index = args.indexOf(flag);
  if (index === -1) return false;
  args.splice(index, 1);
  return true;
};

const takeRequiredValueFlag = (args: string[], flag: string): string => {
  const value = takeValueFlag(args, flag);
  if (value === undefined) throw new Error(`${flag} requires a value`);
  return value;
};

const takeValueFlag = (args: string[], flag: string): string | undefined => {
  const index = args.indexOf(flag);
  if (index === -1) return undefined;
  const value = args[index + 1];
  if (value === undefined || value.startsWith('--')) {
    throw new Error(`${flag} requires a value`);
  }
  args.splice(index, 2);
  return value;
};

const rejectFlags = (args: readonly string[]): void => {
  if (args.length > 0) throw new Error(`Unknown argument: ${args[0]}`);
};

const rejectExtra = (args: readonly string[], expected: number): void => {
  if (args.length > expected)
    throw new Error(`Unknown argument: ${args[expected]}`);
};

const writeJson = (output: Output, value: unknown): void => {
  output.write(`${JSON.stringify(value)}\n`);
};

const usage = [
  'Usage:',
  '  npx nx run mosaic-benchmark:run -- serve <direct|mosaic>',
  '  npx nx run mosaic-benchmark:run -- campaign <skillsbench|terminalbench> check',
  '  npx nx run mosaic-benchmark:run -- campaign <skillsbench|terminalbench> <smoke|run> --yes-paid-run',
  '    Terminal-Bench also requires --skillsbench-report <valid-report.json>',
  '  npx nx run mosaic-benchmark:run -- compare --direct <dir> --mosaic <dir> [--report <file>]',
  '  npx nx run mosaic-benchmark:release',
].join('\n');

const invokedPath = process.argv[1];
if (
  invokedPath !== undefined &&
  import.meta.url === pathToFileURL(invokedPath).href
) {
  process.exitCode = await runCli(process.argv.slice(2));
}
