import { spawn } from 'node:child_process';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

import { resumeCampaign, type ResumeOptions } from './resume.js';
import type { CampaignRun } from './campaign.js';
import { benchmarkRoot } from './root.js';

type Output = Pick<NodeJS.WriteStream, 'write'>;

export type HostDependencies = {
  readonly cwd?: string;
  readonly stdout?: Output;
  readonly stderr?: Output;
  readonly resume?: (options: ResumeOptions) => Promise<CampaignRun>;
  readonly delegate?: (
    args: readonly string[],
    root: string,
  ) => Promise<number>;
};

/** Dispatches host-only commands and delegates released ACP commands unchanged. */
export const runHost = async (
  args: readonly string[],
  dependencies: HostDependencies = {},
): Promise<number> => {
  const root = await benchmarkRoot(dependencies.cwd ?? process.cwd());
  if (!isResume(args)) {
    const code = await (dependencies.delegate ?? delegate)(args, root);
    if (code === 0 && isHelp(args))
      (dependencies.stdout ?? process.stdout).write(`${resumeUsage}\n`);
    return code;
  }

  const stdout = dependencies.stdout ?? process.stdout;
  const stderr = dependencies.stderr ?? process.stderr;
  try {
    const options = resumeOptions(args.slice(3), root);
    const result = await (dependencies.resume ?? resumeCampaign)({
      ...options,
      progress: (stage) => stderr.write(`[mosaic-benchmark] ${stage}\n`),
    });
    stdout.write(`${JSON.stringify(result)}\n`);
    return result.arms.every((arm) => arm.result.code === 0) ? 0 : 1;
  } catch (error) {
    stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    return 2;
  }
};

const isResume = (args: readonly string[]): boolean =>
  args[0] === 'campaign' && args[1] === 'skillsbench' && args[2] === 'resume';
const isHelp = (args: readonly string[]): boolean =>
  args.length === 1 && ['help', '--help', '-h'].includes(args[0] ?? '');
const resumeUsage =
  '  npx nx run mosaic-benchmark:run -- campaign skillsbench resume --campaign <dir> --yes-paid-run';

const resumeOptions = (
  values: readonly string[],
  root: string,
): ResumeOptions => {
  const args = [...values];
  const yesPaidRun = takeBoolean(args, '--yes-paid-run');
  const campaignDir = takeRequired(args, '--campaign');
  const rootDir = takeValue(args, '--root') ?? root;
  if (args.length > 0) throw new Error(`Unknown argument: ${args[0]}`);
  return {
    benchmark: 'skillsbench',
    campaignDir,
    rootDir,
    yesPaidRun,
  };
};

const takeBoolean = (args: string[], flag: string): boolean => {
  const index = args.indexOf(flag);
  if (index === -1) return false;
  args.splice(index, 1);
  return true;
};

const takeRequired = (args: string[], flag: string): string => {
  const value = takeValue(args, flag);
  if (value === undefined) throw new Error(`${flag} requires a value`);
  return value;
};

const takeValue = (args: string[], flag: string): string | undefined => {
  const index = args.indexOf(flag);
  if (index === -1) return undefined;
  const value = args[index + 1];
  if (value === undefined || value.startsWith('--'))
    throw new Error(`${flag} requires a value`);
  args.splice(index, 2);
  return value;
};

const delegate = (args: readonly string[], root: string): Promise<number> =>
  new Promise((resolveResult, reject) => {
    const child = spawn(
      process.execPath,
      [resolve(root, 'dist', 'mosaic-bench-acp.mjs'), ...args],
      { stdio: 'inherit' },
    );
    child.once('error', reject);
    child.once('close', (code) => resolveResult(code ?? 1));
  });

const invokedPath = process.argv[1];
if (
  invokedPath !== undefined &&
  import.meta.url === pathToFileURL(invokedPath).href
)
  process.exitCode = await runHost(process.argv.slice(2));
