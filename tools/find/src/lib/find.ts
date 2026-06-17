import { readdir, readFile } from 'node:fs/promises';
import path from 'node:path';

import { createTool as defineTool } from 'tools';
import { z } from 'zod';

const DEFAULT_LIMIT = 1000;
const MAX_OUTPUT_BYTES = 50 * 1024;

const description =
  'Search for files by glob pattern. Returns matching file paths relative to the search directory. Respects .gitignore. Output is truncated to 1000 results or 50KB (whichever is hit first).';

export const schema = z
  .object({
    pattern: z.string(),
    path: z.string().optional(),
    limit: z.number().int().nonnegative().optional(),
  })
  .strict();

export type FindOutput = {
  readonly results: readonly string[];
  readonly total: number;
  readonly truncated: boolean;
  readonly error?: string;
};

type Options = {
  readonly workspaceRoot: string;
};

type IgnorePattern = {
  readonly base: string;
  readonly pattern: string;
  readonly negated: boolean;
};

/** Creates the provider-neutral file search tool. */
export const createTool = ({ workspaceRoot }: Options) =>
  defineTool({
    name: 'find',
    description,
    schema,
    execute: (input): Promise<FindOutput> => execute(workspaceRoot, input),
  });

const execute = async (
  workspaceRoot: string,
  input: z.output<typeof schema>,
): Promise<FindOutput> => {
  const searchDir = input.path ?? '.';
  const searchPath = resolvePath(workspaceRoot, searchDir);

  if (!(await exists(searchPath))) {
    return empty(`Path not found: ${searchDir}`);
  }

  const glob = compileGlob(input.pattern);
  if (typeof glob === 'string') {
    return empty(`Invalid glob pattern '${input.pattern}': ${glob}`);
  }

  return collect(searchPath, glob, input.limit ?? DEFAULT_LIMIT);
};

const collect = async (
  searchPath: string,
  glob: RegExp,
  limit: number,
): Promise<FindOutput> => {
  const results: string[] = [];
  let totalBytes = 0;
  let totalMatched = 0;
  let truncated = false;

  for await (const file of walk(searchPath, [])) {
    const relative = toPosix(path.relative(searchPath, file));
    const name = path.basename(file);

    if (!glob.test(relative) && !glob.test(name)) {
      continue;
    }

    totalMatched += 1;
    const lineBytes = relative.length + 1;
    if (totalBytes + lineBytes > MAX_OUTPUT_BYTES || results.length >= limit) {
      truncated = true;
      break;
    }

    totalBytes += lineBytes;
    results.push(relative);
  }

  return {
    results,
    total: truncated ? totalMatched : results.length,
    truncated,
  };
};

async function* walk(
  dir: string,
  parentIgnores: readonly IgnorePattern[],
): AsyncGenerator<string> {
  const ignores = [...parentIgnores, ...(await readIgnores(dir))];
  const entries = await readdir(dir, { withFileTypes: true }).catch(() => []);

  for (const entry of entries) {
    if (entry.name === '.git' || entry.name === 'node_modules') {
      continue;
    }

    const fullPath = path.join(dir, entry.name);
    if (isIgnored(fullPath, entry.isDirectory(), ignores)) {
      continue;
    }

    if (entry.isDirectory()) {
      yield* walk(fullPath, ignores);
    } else if (entry.isFile()) {
      yield fullPath;
    }
  }
}

const readIgnores = async (dir: string): Promise<readonly IgnorePattern[]> => {
  const file = path.join(dir, '.gitignore');
  const text = await readFile(file, 'utf8').catch(() => '');

  return text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line !== '' && !line.startsWith('#'))
    .map((line) => ({
      base: dir,
      pattern: line.startsWith('!') ? line.slice(1) : line,
      negated: line.startsWith('!'),
    }))
    .filter((ignore) => ignore.pattern !== '');
};

const isIgnored = (
  fullPath: string,
  isDirectory: boolean,
  ignores: readonly IgnorePattern[],
): boolean => {
  let ignored = false;
  for (const ignore of ignores) {
    const pattern = ignore.pattern.endsWith('/')
      ? ignore.pattern.slice(0, -1)
      : ignore.pattern;
    if (ignore.pattern.endsWith('/') && !isDirectory) {
      continue;
    }

    const glob = compileGlob(pattern);
    const relative = toPosix(path.relative(ignore.base, fullPath));
    const matched =
      glob instanceof RegExp
        ? glob.test(relative) || glob.test(path.basename(fullPath))
        : relative === pattern || path.basename(fullPath) === pattern;
    if (matched) {
      ignored = !ignore.negated;
    }
  }

  return ignored;
};

const compileGlob = (pattern: string): RegExp | string => {
  let source = '^';
  for (let index = 0; index < pattern.length; index += 1) {
    const char = pattern[index];
    const next = pattern[index + 1];

    if (char === '[') {
      return pattern.includes(']', index + 1)
        ? 'unsupported character classes'
        : 'unclosed character class';
    }

    if (char === '*' && next === '*') {
      source += '.*';
      index += 1;
    } else if (char === '*') {
      source += '[^/]*';
    } else if (char === '?') {
      source += '[^/]';
    } else {
      source += escapeRegExp(char);
    }
  }

  return new RegExp(`${source}$`);
};

const resolvePath = (workspaceRoot: string, value: string): string =>
  path.normalize(
    path.isAbsolute(value) ? value : path.join(workspaceRoot, value),
  );

const exists = async (value: string): Promise<boolean> =>
  readdir(value).then(
    () => true,
    async () => {
      const { stat } = await import('node:fs/promises');
      return stat(value).then(
        () => true,
        () => false,
      );
    },
  );

const empty = (error: string): FindOutput => ({
  results: [],
  total: 0,
  truncated: false,
  error,
});

const escapeRegExp = (value: string): string =>
  value.replace(/[\\^$+?.()|{}]/g, '\\$&');
const toPosix = (value: string): string => value.replaceAll(path.sep, '/');
