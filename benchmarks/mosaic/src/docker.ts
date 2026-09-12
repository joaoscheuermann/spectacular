import { spawn } from 'node:child_process';
import { mkdir } from 'node:fs/promises';
import { isAbsolute, relative, resolve, sep } from 'node:path';
import { pathToFileURL } from 'node:url';

import { benchmarkRoot } from './root.js';

type Output = Pick<NodeJS.WriteStream, 'write'>;

export type DockerCommand = {
  readonly file: 'docker';
  readonly args: readonly string[];
  readonly cwd: string;
  readonly environment: NodeJS.ProcessEnv;
};

export type DockerDependencies = {
  readonly cwd?: string;
  readonly environment?: NodeJS.ProcessEnv;
  readonly stderr?: Output;
  readonly execute?: (command: DockerCommand) => Promise<number>;
};

const image = 'mosaic-benchmark:local';
const containerRoot = '/benchmark';
const containerResults = `${containerRoot}/results`;
const resultPathFlags = new Set(['--campaign', '--skillsbench-report']);

/** Builds the Linux coordinator image and runs one benchmark campaign in it. */
export const runDocker = async (
  args: readonly string[],
  dependencies: DockerDependencies = {},
): Promise<number> => {
  const stderr = dependencies.stderr ?? process.stderr;

  try {
    if (args[0] !== 'campaign')
      {throw new Error('docker-run only supports campaign commands.');}

    const cwd = dependencies.cwd ?? process.cwd();
    const root = await benchmarkRoot(cwd);
    const results = resolve(root, 'results');
    const environment = dependencies.environment ?? process.env;
    const execute = dependencies.execute ?? executeCommand;
    const translated = translateArgs(args, { cwd, root, results });

    await mkdir(results, { recursive: true });

    const build = await execute({
      file: 'docker',
      args: ['build', '--tag', image, '.'],
      cwd: root,
      environment,
    });

    if (build !== 0) {return build;}

    return execute({
      file: 'docker',
      args: dockerRunArgs(results, translated, environment),
      cwd: root,
      environment,
    });
  } catch (error) {
    stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);

    return 2;
  }
};

type Paths = {
  readonly cwd: string;
  readonly root: string;
  readonly results: string;
};

const translateArgs = (
  values: readonly string[],
  paths: Paths,
): readonly string[] => {
  const translated: string[] = [];

  for (let index = 0; index < values.length; index += 1) {
    const argument = values[index] ?? '';

    if (!resultPathFlags.has(argument) && argument !== '--root') {
      translated.push(argument);

      continue;
    }

    const value = values[index + 1];

    if (value === undefined || value.startsWith('--'))
      {throw new Error(`${argument} requires a value`);}

    translated.push(
      argument,
      argument === '--root'
        ? translateRoot(value, paths)
        : translateResult(value, paths),
    );

    index += 1;
  }

  return translated;
};

const translateRoot = (value: string, paths: Paths): string => {
  if (resolve(paths.cwd, value) !== paths.root)
    {throw new Error('--root must identify the mosaic-benchmark project.');}

  return containerRoot;
};

const translateResult = (value: string, paths: Paths): string => {
  const child = relative(paths.results, resolve(paths.cwd, value));

  if (
    child === '' ||
    child === '..' ||
    child.startsWith(`..${sep}`) ||
    isAbsolute(child)
  )
    {throw new Error(`Campaign paths must be inside ${paths.results}.`);}

  return `${containerResults}/${child.split(sep).join('/')}`;
};

const dockerRunArgs = (
  results: string,
  args: readonly string[],
  environment: NodeJS.ProcessEnv,
): readonly string[] => [
  'run',
  '--rm',
  '--privileged',
  '--mount',
  `type=bind,source=${results},target=${containerResults}`,
  '--mount',
  'type=volume,source=mosaic-benchmark-docker,target=/var/lib/docker',
  '--mount',
  'type=volume,source=mosaic-benchmark-uv,target=/root/.cache/uv',
  ...(environment.OPENROUTER_API_KEY === undefined
    ? []
    : ['--env', 'OPENROUTER_API_KEY']),
  ...ownershipArgs(),
  image,
  ...args,
];

const ownershipArgs = (): readonly string[] => {
  if (
    typeof process.getuid !== 'function' ||
    typeof process.getgid !== 'function'
  )
    {return [];}

  return [
    '--env',
    `MOSAIC_HOST_UID=${process.getuid()}`,
    '--env',
    `MOSAIC_HOST_GID=${process.getgid()}`,
  ];
};

const executeCommand = (command: DockerCommand): Promise<number> =>
  new Promise((resolveResult, reject) => {
    const child = spawn(command.file, [...command.args], {
      cwd: command.cwd,
      env: command.environment,
      stdio: 'inherit',
    });

    child.once('error', reject);

    child.once('close', (code) => resolveResult(code ?? 1));
  });
const invokedPath = process.argv[1];

if (
  invokedPath !== undefined &&
  import.meta.url === pathToFileURL(invokedPath).href
)
  {process.exitCode = await runDocker(process.argv.slice(2));}
